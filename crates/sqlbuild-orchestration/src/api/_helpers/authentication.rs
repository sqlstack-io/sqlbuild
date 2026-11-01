//! Authenticate before body buffering; scope checks remain in each owning endpoint.

use std::net::SocketAddr;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{DevelopmentScope, ReadResource};
use crate::credentials::main::authenticate_token::authenticate_token;
use crate::credentials::models::{CredentialScope, TokenIdentity};
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn authenticate(
    State(store): State<ControlStore>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let development = request.extensions().get::<DevelopmentScope>().cloned();
    let identity = match bearer(request.headers()) {
        Ok(token) => authenticate_token(&store, token).await?,
        Err(_) if development.is_some() => None,
        Err(error) => return Err(error),
    };
    let identity = identity
        .or_else(|| {
            development.map(|scope| TokenIdentity {
                token_id: Uuid::nil(),
                scope: CredentialScope::Location {
                    location_id: scope.location_id,
                },
                expires_at: chrono::DateTime::<chrono::Utc>::MAX_UTC,
            })
        })
        .ok_or_else(unauthorized)?;
    request.extensions_mut().insert(identity);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

pub(crate) fn bearer(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(header::AUTHORIZATION)
        .ok_or_else(unauthorized)?
        .to_str()
        .map_err(|_| unauthorized())?
        .strip_prefix("Bearer ")
        .ok_or_else(unauthorized)
}

fn unauthorized() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "valid bearer credential required".into(),
    }
}

/// Reject browser cross-origin and DNS-rebinding requests on the disabled-auth listener.
pub(crate) async fn local_request(
    State(address): State<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let authorities = [address.to_string(), format!("localhost:{}", address.port())];
    let valid_host = request.headers().get(header::HOST).is_some_and(|value| {
        value
            .to_str()
            .is_ok_and(|host| authorities.iter().any(|allowed| host == allowed))
    });
    let valid_origin = request.headers().get(header::ORIGIN).is_none_or(|value| {
        value.to_str().is_ok_and(|origin| {
            authorities
                .iter()
                .any(|host| origin == format!("http://{host}"))
        })
    });
    if !valid_host || !valid_origin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "development API requires a local authority and origin".into(),
        });
    }
    Ok(next.run(request).await)
}

pub(crate) fn require_location(
    identity: &TokenIdentity,
    location_id: Uuid,
) -> Result<(), ApiError> {
    if identity.scope != (CredentialScope::Location { location_id }) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "credential does not authorize this location".into(),
        });
    }
    Ok(())
}

/// A guessed resource ID never widens the already authenticated location and target.
pub(crate) async fn require_resource(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    (resource, id): (ReadResource, Uuid),
) -> Result<(), ApiError> {
    let (statement, missing) = match resource {
        ReadResource::Run => (
            "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
            "run not found",
        ),
        ReadResource::Trigger => (
            "SELECT trigger_id FROM sqb_triggers WHERE trigger_id=$1 AND location_id=$2 AND target=$3",
            "trigger not found",
        ),
    };
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        statement,
        vec![
            Parameter::Id(id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    commit(transaction).await?;
    if rows.is_empty() {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: missing.into(),
        });
    }
    Ok(())
}
