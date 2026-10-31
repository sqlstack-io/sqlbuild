//! Do not echo response bodies: server diagnostics can contain user configuration.

use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;

use crate::worker::errors::WorkerError;

/// Each runtime owns its connection pool; sharing pools could tie heartbeat IO to build work.
pub(crate) fn http_client(authorization: HeaderValue) -> Result<reqwest::Client, WorkerError> {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, authorization);
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()?)
}

pub(crate) async fn response<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<T, WorkerError> {
    let response = request.send().await.map_err(reqwest::Error::without_url)?;
    if !response.status().is_success() {
        return Err(WorkerError::Rejected(response.status()));
    }
    Ok(response.json().await.map_err(reqwest::Error::without_url)?)
}
