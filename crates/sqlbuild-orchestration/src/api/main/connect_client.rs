//! Validate the API origin before attaching a sensitive bearer credential.

use reqwest::header::HeaderValue;

use crate::api::_helpers::client::http_client;
use crate::api::errors::ClientError;
use crate::api::models::ControlClient;

pub(crate) fn connect_client(server_url: &str, bearer: &str) -> Result<ControlClient, ClientError> {
    let base_url = reqwest::Url::parse(server_url)
        .map_err(|_| ClientError::Configuration("server URL must be an absolute HTTP URL"))?;
    if !matches!(base_url.scheme(), "http" | "https")
        || base_url.host_str().is_none()
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(ClientError::Configuration(
            "server URL must use HTTP or HTTPS without credentials, query or fragment",
        ));
    }
    let mut authorization = HeaderValue::from_str(&format!("Bearer {bearer}"))
        .map_err(|_| ClientError::Configuration("bearer is not a valid HTTP credential"))?;
    authorization.set_sensitive(true);
    Ok(ControlClient {
        client: http_client(authorization.clone())?,
        authorization,
        base_url,
    })
}
