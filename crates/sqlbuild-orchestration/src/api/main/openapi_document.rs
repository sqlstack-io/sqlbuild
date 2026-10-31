//! Generate the HTTP contract from the request/response types used by the actual endpoints.

use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

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
}
