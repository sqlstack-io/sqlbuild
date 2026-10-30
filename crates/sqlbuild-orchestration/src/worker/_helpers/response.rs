//! Do not echo response bodies: server diagnostics can contain user configuration.

use serde::de::DeserializeOwned;

use crate::worker::errors::WorkerError;

pub(crate) async fn response<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<T, WorkerError> {
    let response = request.send().await.map_err(reqwest::Error::without_url)?;
    if !response.status().is_success() {
        return Err(WorkerError::Rejected(response.status()));
    }
    Ok(response.json().await.map_err(reqwest::Error::without_url)?)
}
