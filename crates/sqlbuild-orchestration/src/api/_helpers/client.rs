//! Each client runtime owns its pool; credentials and server response bodies stay out of diagnostics.

use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::api::errors::ClientError;
use crate::api::models::ControlClient;

pub(crate) fn http_client(authorization: HeaderValue) -> Result<reqwest::Client, ClientError> {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, authorization);
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()?)
}

impl ControlClient {
    pub(crate) fn independent(&self) -> Result<Self, ClientError> {
        Ok(Self {
            client: http_client(self.authorization.clone())?,
            authorization: self.authorization.clone(),
            base_url: self.base_url.clone(),
        })
    }

    pub(crate) fn endpoint(&self, segments: &[&str]) -> Result<reqwest::Url, ClientError> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| ClientError::Configuration("server URL cannot contain an API path"))?
            .pop_if_empty()
            .extend(segments.iter().copied());
        Ok(url)
    }
}
