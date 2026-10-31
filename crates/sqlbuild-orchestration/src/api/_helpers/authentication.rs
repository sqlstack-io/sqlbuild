//! Authenticate before body buffering; scope checks remain in each owning endpoint.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::ReadResource;
use crate::credentials::main::authenticate_token::authenticate_token;
use crate::credentials::models::{CredentialScope, TokenIdentity};
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

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
