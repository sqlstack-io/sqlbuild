//! Share bounded JSON transport without exposing server bodies or credential-bearing URLs.

use serde::de::DeserializeOwned;

use crate::api::errors::ClientError;

pub(crate) async fn client_response<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<T, ClientError> {
    let response = request.send().await.map_err(reqwest::Error::without_url)?;
    if !response.status().is_success() {
        return Err(ClientError::Rejected(response.status()));
    }
    Ok(response.json().await.map_err(reqwest::Error::without_url)?)
}
