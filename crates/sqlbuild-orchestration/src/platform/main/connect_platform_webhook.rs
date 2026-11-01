//! Validate operator-owned delivery configuration without exposing credentials in errors.

use std::time::Duration;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::platform::constants::SINK_REQUEST_TIMEOUT_SECONDS;
use crate::platform::models::PlatformWebhook;
use crate::store::errors::ControlError;

/// Changing the endpoint creates a new consumer, starting from the retained event history.
pub fn connect_platform_webhook(endpoint: &str) -> Result<PlatformWebhook, ControlError> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|_| ControlError::Invalid("platform webhook needs an HTTP(S) URL".into()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ControlError::Invalid(
            "platform webhook needs an HTTP(S) URL".into(),
        ));
    }
    let sink_id = sha256_hex(url.as_str().as_bytes());
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(SINK_REQUEST_TIMEOUT_SECONDS))
        .build()
        .map_err(|_| ControlError::Invalid("platform webhook client could not start".into()))?;
    Ok(PlatformWebhook {
        client,
        url,
        sink_id,
    })
}
