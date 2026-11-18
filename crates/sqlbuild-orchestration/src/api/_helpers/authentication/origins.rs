//! The browser origin every sign-in method checks requests against.

use crate::api::constants::{HTTP_SCHEME, HTTPS_SCHEME, ROOT_PATH};
use crate::store::errors::ControlError;

/// A bare `https://` origin, or with `allow_http` also an `http://` one, in its serialized form.
pub(crate) fn parse_origin_with(
    value: &str,
    mechanism: &str,
    allow_http: bool,
) -> Result<String, ControlError> {
    let schemes = if allow_http {
        "an HTTP or HTTPS"
    } else {
        "an HTTPS"
    };
    let error = || {
        ControlError::Invalid(format!(
            "{mechanism} authentication requires {schemes} origin without credentials, path, query or fragment"
        ))
    };
    let url = reqwest::Url::parse(value).map_err(|_| {
        ControlError::Invalid(format!(
            "{mechanism} authentication requires {schemes} origin"
        ))
    })?;
    let scheme_allowed = url.scheme() == HTTPS_SCHEME || allow_http && url.scheme() == HTTP_SCHEME;
    if !scheme_allowed
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
