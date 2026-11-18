//! Starts proxy-authenticated APIs and sends requests the way a host-rewriting proxy forwards them.

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::ConnectInfo;
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::_helpers::authentication::configure::configure;
use crate::api::models::{
    ApiAuthentication, HumanLogin, TrustedProxyAuthentication, UnknownProxyUsers,
};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::test_types::TestApi;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) const ORIGIN: &str = "http://control.example.com";
pub(crate) const PUBLIC_HOST: &str = "control.example.com";
pub(crate) const USER_HEADER: &str = "x-example-user";
pub(crate) const NAME_HEADER: &str = "x-example-name";
pub(crate) const LOGOUT_URL: &str = "https://sign-in.example.com/logout";

pub(crate) fn settings(
    trusted_peer: &str,
    unknown_users: UnknownProxyUsers,
) -> TrustedProxyAuthentication {
    TrustedProxyAuthentication {
        origin: ORIGIN.into(),
        allowed_hosts: Vec::new(),
        trusted_peers: BTreeSet::from([trusted_peer.parse().expect("peer IP")]),
        identities: BTreeMap::new(),
        username_header: "X-Example-User".into(),
        display_name_header: Some("X-Example-Name".into()),
        email_header: None,
        unknown_users,
        proxy_logout_url: Some(LOGOUT_URL.into()),
    }
}

/// `[trusted_proxy]` with its origin and `allowed_hosts`, so `trusted_peers` takes its loopback default.
pub(crate) fn proxy_login(allowed_hosts: &[&str]) -> HumanLogin {
    HumanLogin::TrustedProxy(
        serde_json::from_value(json!({
            "origin": ORIGIN,
            "username_header": USER_HEADER,
            "allowed_hosts": allowed_hosts,
        }))
        .expect("proxy settings with the default peers"),
    )
}

/// Sends `request` through the sign-in routes `login` configures, as if it arrived from `peer`.
pub(crate) async fn send_from_peer(
    store: &ControlStore,
    login: HumanLogin,
    (peer, mut request): (&str, axum::http::Request<axum::body::Body>),
) -> axum::response::Response {
    let (_, routes, _) = configure(
        store,
        ApiAuthentication {
            login,
            ..ApiAuthentication::default()
        },
        "127.0.0.1:8080".parse().expect("listener"),
    )
    .await
    .expect("sign-in routes");
    let peer = std::net::SocketAddr::new(peer.parse().expect("peer IP"), 40_000);
    request.extensions_mut().insert(ConnectInfo(peer));
    routes
        .with_state(store.clone())
        .oneshot(request)
        .await
        .expect("response")
}

pub(crate) async fn start(store: &ControlStore, settings: TrustedProxyAuthentication) -> TestApi {
    start_api_with_authentication(
        store,
        ApiAuthentication {
            login: HumanLogin::TrustedProxy(settings),
            ..ApiAuthentication::default()
        },
    )
    .await
}

/// What the proxy forwards: the backend address as `Host` and the browser's host in `X-Forwarded-Host`.
pub(crate) fn forwarded(client: &Client, method: reqwest::Method, url: String) -> RequestBuilder {
    client
        .request(method, url)
        .header("x-forwarded-host", PUBLIC_HOST)
}

pub(crate) async fn rows(store: &ControlStore, sql: &str, parameters: Vec<Parameter>) -> Value {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        sql,
        parameters,
    )
    .await
    .expect("query");
    commit(transaction).await.expect("read complete");
    json!(rows)
}
