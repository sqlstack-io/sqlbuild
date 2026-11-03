//! Browser authentication and CLI handoffs share one strict HTTPS origin contract.

use crate::api::constants::{HTTPS_SCHEME, ROOT_PATH};
use crate::store::errors::ControlError;

pub fn parse_origin(value: &str, mechanism: &str) -> Result<String, ControlError> {
    let error = || {
        ControlError::Invalid(format!(
            "{mechanism} authentication requires an HTTPS origin without credentials, path, query or fragment"
        ))
    };
    let url = reqwest::Url::parse(value).map_err(|_| {
        ControlError::Invalid(format!(
            "{mechanism} authentication requires an HTTPS origin"
        ))
    })?;
    if url.scheme() != HTTPS_SCHEME
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != ROOT_PATH
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error());
    }
    Ok(url.origin().ascii_serialization())
}
