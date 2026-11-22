//! Sign in over real HTTP as the people the password-people tests need.

use reqwest::header::{COOKIE, HOST, ORIGIN, SET_COOKIE};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::PasswordSessionResponse;
use crate::authorization::main::roles::grant_admin::grant_admin;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::main::sessions::set_password::set_password;
use crate::credentials::models::{AuditContext, PasswordAuthority};
use crate::store::models::ControlStore;

pub(crate) const ORIGIN_URL: &str = "https://control.example.com";
pub(crate) const PASSWORD: &str = "a long enough password";
pub(crate) const NEW_PASSWORD: &str = "the newcomer's first password";

pub(crate) fn context() -> AuditContext {
    AuditContext {
        actor: "test-admin".into(),
        granting_role: "admin".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    }
}

pub(crate) fn browser() -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(HOST, "control.example.com".parse().expect("host"));
    headers.insert(ORIGIN, ORIGIN_URL.parse().expect("origin"));
    Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("browser transport")
}

/// A person who signs in with `PASSWORD`.
pub(crate) async fn password_person(store: &ControlStore, name: &str) -> Uuid {
    let id = create_principal(store, &context(), name)
        .await
        .expect("person");
    set_password(store, &context(), PasswordAuthority::Operator(id), PASSWORD)
        .await
        .expect("password");
    id
}

/// An administrator who signs in with `PASSWORD`.
pub(crate) async fn password_administrator(store: &ControlStore, name: &str) {
    let id = password_person(store, name).await;
    grant_admin(store, &context(), id)
        .await
        .expect("administrator");
}

/// How the server answers signing in as `name` with `password`.
pub(crate) async fn sign_in_status(
    client: &Client,
    url: &str,
    (name, password): (&str, &str),
) -> StatusCode {
    client
        .post(format!("{url}/api/auth/password/login"))
        .json(&json!({"username": name, "password": password}))
        .send()
        .await
        .expect("login")
        .status()
}

/// The session cookie and CSRF token of signing in as `name` with `PASSWORD`.
pub(crate) async fn session(client: &Client, url: &str, name: &str) -> (String, String) {
    let response = client
        .post(format!("{url}/api/auth/password/login"))
        .json(&json!({"username": name, "password": PASSWORD}))
        .send()
        .await
        .expect("login")
        .error_for_status()
        .expect("signed in");
    let cookie = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("cookie"))
        .find(|value| value.starts_with("__Host-sqb-session="))
        .and_then(|value| value.split(';').next())
        .expect("session cookie")
        .to_owned();
    let issued: PasswordSessionResponse = response.json().await.expect("session");
    (cookie, issued.csrf_token)
}

/// The built-in roles `name` holds everywhere, read by the administrator.
pub(crate) async fn roles_of(
    client: &Client,
    url: &str,
    admin: &(String, String),
    name: &str,
) -> Vec<String> {
    let holdings: Value = client
        .get(format!("{url}/api/admin/roles"))
        .header(COOKIE, &admin.0)
        .send()
        .await
        .expect("role holdings")
        .json()
        .await
        .expect("holdings body");
    let mut roles: Vec<String> = holdings["assignments"]
        .as_array()
        .expect("assignments")
        .iter()
        .filter(|held| held["name"] == name && held["location_id"].is_null())
        .map(|held| held["role_key"].as_str().expect("role").to_owned())
        .collect();
    roles.sort();
    roles
}
