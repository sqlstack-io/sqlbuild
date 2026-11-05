//! Validate operator-owned delivery configuration without exposing credentials in errors.

use std::time::Duration;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::errors::ControlError;
use crate::webhooks::constants::REQUEST_TIMEOUT_SECONDS;
use crate::webhooks::models::{Webhook, WebhookSource};

/// Changing the endpoint creates a new consumer, starting from the retained event history.
pub fn connect_webhook(endpoint: &str, source: WebhookSource) -> Result<Webhook, ControlError> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|_| ControlError::Invalid("webhook needs an HTTP(S) URL".into()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ControlError::Invalid("webhook needs an HTTP(S) URL".into()));
    }
    let sink_id = sha256_hex(url.as_str().as_bytes());
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .build()
        .map_err(|_| ControlError::Invalid("webhook client could not start".into()))?;
    Ok(Webhook {
        client,
        url,
        sink_id,
        source,
    })
}
