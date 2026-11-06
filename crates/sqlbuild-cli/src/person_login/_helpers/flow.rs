//! A CLI-only random secret can be exchanged only after an explicit browser decision.

use std::fmt::Write;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::person_login::constants::{LOGIN_SECONDS, POLL_SECONDS, SECRET_BYTES};
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

fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}
