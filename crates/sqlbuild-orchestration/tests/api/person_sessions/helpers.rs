//! Authenticate real browser transports and observe their durable queue effects.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, Method, Response, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, OidcAuthentication, TrustedProxyAuthentication};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::oidc_helpers::{callback, login};
use crate::api::tests::oidc_test_types::TestProvider;
use crate::api::tests::person_sessions_test_types::BrowserSession;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) async fn browser_sessions(
    store: &ControlStore,
    provider: &TestProvider,
    person: Uuid,
) -> Vec<BrowserSession> {
    let origin = "https://control.example.com";
    let password = start_api_with_authentication(
        store,
        ApiAuthentication {
            password_origin: Some(origin.into()),
            ..Default::default()
        },
    )
    .await;
    let proxy = start_api_with_authentication(
        store,
        ApiAuthentication {
            trusted_proxy: Some(TrustedProxyAuthentication {
                origin: origin.into(),
                trusted_peers: BTreeSet::from(["127.0.0.1".parse().expect("socket peer")]),
                identities: BTreeMap::from([("orders-person".into(), person)]),
            }),
            ..Default::default()
        },
    )
    .await;
    let oidc = start_api_with_authentication(
        store,
        ApiAuthentication {
            oidc: Some(OidcAuthentication {
                origin: origin.into(),
                issuer: provider.state.issuer.clone(),
                client_id: "orders-control".into(),
                client_secret_env: None,
                certificate_authority: Some(provider.authority.clone()),
                identities: BTreeMap::from([("orders-person".into(), person)]),
            }),
            ..Default::default()
        },
    )
    .await;
    let password_response = provider
        .client
        .post(format!("{}/api/auth/password/login", password.url))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, origin)
        .json(&json!({"username":"orders-person", "password":"orders-password-2026"}))
        .send()
        .await
        .expect("password login");
    let flow = login(provider, &oidc.url).await;
    let oidc_response = callback(provider, &oidc.url, &flow).await;
    let mut sessions = Vec::new();
    for (api, response, identity_path) in [
        (password, password_response, "/identity/principal_id"),
        (oidc, oidc_response, "/principal_id"),
    ] {
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().expect("cookie"))
            .find(|value| value.starts_with("__Host-sqb-session="))
            .expect("browser session")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned();
        let body: Value = response.json().await.expect("session response");
        assert_eq!(body.pointer(identity_path), Some(&json!(person)));
        assert!(body.get("bearer").is_none());
        sessions.push(BrowserSession {
            api,
            credential: ("cookie", cookie),
            csrf: body["csrf_token"].as_str().expect("CSRF").into(),
            forgery: ("x-sqb-csrf", "wrong-csrf"),
        });
    }
    sessions.push(BrowserSession {
        api: proxy,
        credential: ("x-sqb-user", "orders-person".into()),
        csrf: String::new(),
        forgery: ("origin", "https://other.example.com"),
    });
    sessions
}

pub(crate) async fn request(
    client: &Client,
    session: &BrowserSession,
    (method, path): (Method, &str),
    (body, csrf): (&Value, &str),
) -> Response {
    client
        .request(method, format!("{}{path}", session.api.url))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, "https://control.example.com")
        .header(session.credential.0, &session.credential.1)
        .header("x-sqb-csrf", csrf)
        .json(body)
        .send()
        .await
        .expect("browser operation")
}

/// The request a cross-site page could forge: its CSRF proof or origin is wrong.
pub(crate) async fn forged_request(
    client: &Client,
    session: &BrowserSession,
    (method, path): (Method, &str),
    body: &Value,
) -> Response {
    client
        .request(method, format!("{}{path}", session.api.url))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, "https://control.example.com")
        .header(session.credential.0, &session.credential.1)
        .header("x-sqb-csrf", &session.csrf)
        .header(session.forgery.0, session.forgery.1)
        .json(body)
        .send()
        .await
        .expect("forged browser operation")
}

pub(crate) async fn queue_counts(store: &ControlStore) -> Value {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("inspect queues");
    let (transaction, rows) = query(
        transaction,
        "SELECT (SELECT count(*) FROM sqb_location_work) AS work, \
         (SELECT count(*) FROM sqb_runs) AS runs, \
         (SELECT count(*) FROM sqb_write_claims) AS claims",
        vec![],
    )
    .await
    .expect("queue counts");
    commit(transaction).await.expect("close inspection");
    json!(rows[0])
}
