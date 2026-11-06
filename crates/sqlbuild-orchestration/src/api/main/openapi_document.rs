//! Generate the HTTP contract from the request/response types used by the actual endpoints.

use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};

use crate::api::constants::{PASSWORD_COOKIE, PROXY_USER_HEADER};
use crate::api::models::ControlApi;

#[must_use]
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    let mut document = ControlApi::openapi();
    document
        .components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
    document
        .components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(PASSWORD_COOKIE))),
        );
    document
        .components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            "cliSession",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "Authorization",
                "Session <credential> from sqb login; never combine with Cookie. Writes also require x-sqb-csrf.",
            ))),
        );
    document
        .components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            "proxy",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                PROXY_USER_HEADER,
                "Upstream-replaced subject; only accepted from explicitly configured socket peers",
            ))),
        );
    document
}
