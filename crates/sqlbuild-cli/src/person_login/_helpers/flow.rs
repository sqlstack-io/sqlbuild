//! A CLI-only random secret can be exchanged only after an explicit browser decision.

use std::fmt::Write;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::person_login::constants::{KEYS_ONLY_MODE, LOGIN_SECONDS, POLL_SECONDS, SECRET_BYTES};
use crate::person_login::main::open_browser::open_browser;
use crate::person_login::models::{LoginFlow, PersonClient, PersonCredential};

pub(crate) async fn login(
    client: &PersonClient,
    no_browser: bool,
    report: &impl Fn(&str),
) -> Result<PersonCredential, std::io::Error> {
    let mut bytes = [0_u8; SECRET_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| std::io::Error::other("could not generate a secure CLI exchange secret"))?;
    let mut secret = String::new();
    for byte in bytes {
        write!(secret, "{byte:02x}").map_err(std::io::Error::other)?;
    }
    let response = client
        .client
        .post(format!("{}/api/auth/cli/start", client.origin))
        .json(&json!({"client_hash":sha256_hex(secret.as_bytes())}))
        .send()
        .await
        .map_err(|_| std::io::Error::other("could not start browser sign-in"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND && keys_only(client).await {
        return Err(std::io::Error::other(
            "this server has no sign-in for people: scripts call it with a service account key",
        ));
    }
    if !response.status().is_success() {
        return Err(std::io::Error::other(
            "server rejected CLI sign-in; check its authentication configuration or wait before retrying",
        ));
    }
    let flow: LoginFlow = response
        .json()
        .await
        .map_err(|_| std::io::Error::other("server returned an invalid CLI handoff"))?;
    let (id, code) = flow
        .browser_code
        .split_once('.')
        .ok_or_else(|| std::io::Error::other("server returned an invalid CLI handoff"))?;
    let id = uuid::Uuid::parse_str(id)
        .map_err(|_| std::io::Error::other("server returned an invalid CLI handoff"))?;
    if code.len() != SECRET_BYTES * 2 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(std::io::Error::other(
            "server returned an invalid CLI handoff",
        ));
    }
    let url = format!("{}/auth/cli#{}", client.origin, flow.browser_code);
    report(&format!(
        "Open {url}\nVerification code: {}\nWaiting for browser authorization…\n",
        code[..8].to_ascii_uppercase()
    ));
    if !no_browser && !open_browser(&url) {
        report("Could not open a browser automatically. Open the URL above manually.\n");
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(LOGIN_SECONDS) && Utc::now() < flow.expires_at {
        let response = client
            .client
            .post(format!("{}/api/auth/cli/claim", client.origin))
            .json(&json!({"flow_id":id,"client_secret":secret}))
            .send()
            .await
            .map_err(|_| {
                std::io::Error::other("could not complete CLI sign-in; start sqb login again")
            })?;
        match response.status() {
            reqwest::StatusCode::ACCEPTED => {
                tokio::time::sleep(Duration::from_secs(POLL_SECONDS)).await
            }
            reqwest::StatusCode::OK => {
                return response.json().await.map_err(|_| {
                    std::io::Error::other("server returned an invalid CLI credential")
                });
            }
            _ => {
                return Err(std::io::Error::other(
                    "CLI sign-in was cancelled, expired or is unavailable; start sqb login again",
                ));
            }
        }
    }
    Err(std::io::Error::other(
        "CLI sign-in expired; start sqb login again",
    ))
}

/// Whether the server answers that only service account keys call it.
async fn keys_only(client: &PersonClient) -> bool {
    let Ok(response) = client
        .client
        .get(format!("{}/api/auth/login-mode", client.origin))
        .send()
        .await
    else {
        return false;
    };
    let answer: serde_json::Value = response.json().await.unwrap_or_default();
    answer["login_mode"] == KEYS_ONLY_MODE
}
