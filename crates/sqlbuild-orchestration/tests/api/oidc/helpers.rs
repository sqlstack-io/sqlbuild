//! Real TLS discovery, JWKS, authorization and code exchange for native HTTP acceptance.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use openidconnect::core::{
    CoreEdDsaPrivateSigningKey, CoreGenderClaim, CoreIdTokenClaims, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    AccessToken, Audience, EmptyAdditionalClaims, IdTokenClaims, IssuerUrl, JsonWebKeyId, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::OidcExtraClaims;
use crate::api::models::{ApiAuthentication, HumanLogin, OidcAuthentication};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::oidc_test_types::{BrowserFlow, ProviderFault, ProviderState, TestProvider};
use crate::api::types::OidcIdToken;
use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

pub(crate) async fn start_provider() -> TestProvider {
    let directory = tempfile::tempdir().expect("disposable provider certificates");
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])
        .expect("fresh TLS key and certificate");
    let pem = certificate.cert.pem();
    let authority = directory.path().join("orders-ca.pem");
    std::fs::write(&authority, &pem).expect("fixture authority");
    let tls = axum_server::tls_rustls::RustlsConfig::from_pem(
        pem.as_bytes().to_vec(),
        certificate.signing_key.serialize_pem().into_bytes(),
    )
    .await
    .expect("TLS configuration");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("provider socket");
    listener
        .set_nonblocking(true)
        .expect("asynchronous TLS listener");
    let port = listener.local_addr().expect("provider address").port();
    let key = |id: &str| {
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ED25519).expect("fresh signing key");
        Arc::new(
            CoreEdDsaPrivateSigningKey::from_ed25519_pem(
                &key.serialize_pem(),
                Some(JsonWebKeyId::new(id.into())),
            )
            .expect("OIDC signing key"),
        )
    };
    let state = ProviderState {
        issuer: format!("https://localhost:{port}"),
        keys: Arc::new(BTreeMap::from([
            ("orders".into(), key("orders")),
            ("customers".into(), key("customers")),
        ])),
        published_key: Arc::new(Mutex::new("orders".into())),
        codes: Arc::new(Mutex::new(BTreeMap::new())),
        fault: Arc::new(Mutex::new(ProviderFault::default())),
        subject: Arc::new(Mutex::new("orders-person".into())),
    };
    let router = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .with_state(state.clone());
    let handle = axum_server::Handle::new();
    let server = axum_server::from_tcp_rustls(listener, tls)
        .expect("TLS listener")
        .handle(handle.clone());
    let task = tokio::spawn(server.serve(router.into_make_service()));
    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(pem.as_bytes()).expect("root"))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("verified TLS client");
    TestProvider {
        state,
        authority,
        client,
        handle,
        task,
        directory,
    }
}

pub(crate) async fn stop_provider(provider: TestProvider) {
    provider
        .handle
        .graceful_shutdown(Some(std::time::Duration::from_secs(1)));
    provider
        .task
        .await
        .expect("join provider")
        .expect("provider shutdown");
    provider
        .directory
        .close()
        .expect("remove disposable provider keys");
}

async fn discovery(State(state): State<ProviderState>) -> Json<Value> {
    Json(json!({
        "issuer": state.issuer,
        "authorization_endpoint": format!("{}/authorize", state.issuer),
        "token_endpoint": format!("{}/token", state.issuer),
        "jwks_uri": format!("{}/jwks", state.issuer),
        "response_types_supported": ["code"], "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "code_challenge_methods_supported": ["S256"]
    }))
}

async fn jwks(State(state): State<ProviderState>) -> Json<Value> {
    let key = state.published_key.lock().expect("published key");
    Json(json!({"keys": [state.keys[key.as_str()].as_verification_key()]}))
}

async fn authorize(
    State(state): State<ProviderState>,
    Query(request): Query<BTreeMap<String, String>>,
) -> Redirect {
    assert_eq!(request["client_id"], "orders-control");
    assert_eq!(request["response_type"], "code");
    assert_eq!(request["code_challenge_method"], "S256");
    assert!(
        request["scope"]
            .split_whitespace()
            .any(|scope| scope == "openid")
    );
    let code = Uuid::now_v7().to_string();
    let mut callback = reqwest::Url::parse(&request["redirect_uri"]).expect("callback URI");
    callback
        .query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &request["state"]);
    state.codes.lock().expect("code lock").insert(code, request);
    Redirect::to(callback.as_str())
}

async fn token(State(state): State<ProviderState>, body: String) -> Json<Value> {
    let form = reqwest::Url::parse(&format!("https://provider.example.com/?{body}"))
        .expect("token form")
        .query_pairs()
        .into_owned()
        .collect::<BTreeMap<_, _>>();
    assert_eq!(form["grant_type"], "authorization_code");
    let request = state
        .codes
        .lock()
        .expect("code lock")
        .remove(&form["code"])
        .expect("one-use authorization code");
    assert_eq!(form["redirect_uri"], request["redirect_uri"]);
    let challenge = PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
        form["code_verifier"].clone(),
    ));
    assert_eq!(challenge.as_str(), request["code_challenge"]);
    let fault = state.fault.lock().expect("fault lock").clone();
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(state.issuer.clone()).expect("issuer"),
        vec![Audience::new("orders-control".into())],
        Utc::now() + Duration::seconds(300),
        Utc::now(),
        StandardClaims::new(SubjectIdentifier::new(
            state.subject.lock().expect("subject").clone(),
        )),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(request["nonce"].clone())));
    let mut claims = serde_json::to_value(claims).expect("claims JSON");
    claims
        .as_object_mut()
        .expect("claims object")
        .extend(fault.claims);
    let claims = serde_json::from_value::<IdTokenClaims<OidcExtraClaims, CoreGenderClaim>>(claims)
        .expect("fixture claims");
    let signing = &state.keys[fault.signing_key.as_deref().unwrap_or("orders")];
    let id_token = OidcIdToken::new(
        claims,
        signing.as_ref(),
        CoreJwsSigningAlgorithm::EdDsa,
        Some(&AccessToken::new("orders-access".into())),
        None,
    )
    .expect("signed token");
    let mut response = json!({"token_type":"Bearer", "expires_in":300,
        "access_token": "orders-access",
        "id_token": id_token.to_string()});
    response
        .as_object_mut()
        .expect("response")
        .extend(fault.response);
    for field in fault.remove_fields {
        response.as_object_mut().expect("response").remove(&field);
    }
    Json(response)
}

pub(crate) async fn login(provider: &TestProvider, url: &str) -> BrowserFlow {
    let response = provider
        .client
        .get(format!("{url}/api/auth/oidc/login"))
        .header(reqwest::header::HOST, "control.example.com")
        .send()
        .await
        .expect("login");
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    let cookie = response.headers()[reqwest::header::SET_COOKIE]
        .to_str()
        .expect("cookie");
    for attribute in [
        "Secure",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=300",
    ] {
        assert!(cookie.contains(attribute), "{attribute}");
    }
    assert!(!cookie.contains("Domain="));
    let cookie = cookie.split(';').next().expect("browser binding").into();
    let authorize = response.headers()[reqwest::header::LOCATION]
        .to_str()
        .expect("authorization URL");
    let response = provider
        .client
        .get(authorize)
        .send()
        .await
        .expect("provider authorize");
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    let callback = reqwest::Url::parse(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .expect("callback"),
    )
    .expect("callback URL");
    BrowserFlow { cookie, callback }
}

pub(crate) async fn callback(
    provider: &TestProvider,
    url: &str,
    flow: &BrowserFlow,
) -> reqwest::Response {
    let mut endpoint = reqwest::Url::parse(url).expect("API URL");
    endpoint.set_path(flow.callback.path());
    endpoint.set_query(flow.callback.query());
    provider
        .client
        .get(endpoint)
        .header(reqwest::header::HOST, "control.example.com")
        .header(reqwest::header::COOKIE, &flow.cookie)
        .send()
        .await
        .expect("callback response")
}

pub(crate) async fn start_oidc_api(
    store: &ControlStore,
    settings: &OidcAuthentication,
) -> crate::api::tests::test_types::TestApi {
    start_api_with_authentication(
        store,
        ApiAuthentication {
            login: HumanLogin::Oidc(settings.clone()),
            ..Default::default()
        },
    )
    .await
}

pub(crate) async fn sign_in(provider: &TestProvider, url: &str, fault: ProviderFault) {
    *provider.state.fault.lock().expect("fault lock") = fault;
    let flow = login(provider, url).await;
    assert_eq!(
        callback(provider, url, &flow).await.status(),
        reqwest::StatusCode::OK
    );
}

pub(crate) async fn authorize_in(
    store: &ControlStore,
    person: Uuid,
    (action, location_id): (Action, Uuid),
) -> Result<String, ControlError> {
    let context = AuditContext {
        actor: format!("principal:{person}"),
        granting_role: "browser-session".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let scope = PermissionScope {
        action,
        location_id: Some(location_id),
        target: Some("development".into()),
        nodes: Default::default(),
    };
    let (transaction, context) =
        begin_authorized(store, (&context, person), &scope, DefaultRole::None).await?;
    commit(transaction).await?;
    Ok(context.granting_role)
}
