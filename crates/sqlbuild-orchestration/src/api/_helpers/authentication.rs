//! Authenticate before body buffering; scope checks remain in each owning endpoint.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::api::errors::ApiError;
use crate::credentials::main::authenticate_token::authenticate_token;
use crate::store::models::ControlStore;

pub(crate) async fn authenticate(
    State(store): State<ControlStore>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let unauthorized = || ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "valid bearer credential required".into(),
    };
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(unauthorized)?
        .to_str()
        .map_err(|_| unauthorized())?
        .strip_prefix("Bearer ")
        .ok_or_else(unauthorized)?;
    let identity = authenticate_token(&store, bearer)
        .await?
        .ok_or_else(unauthorized)?;
    request.extensions_mut().insert(identity);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
