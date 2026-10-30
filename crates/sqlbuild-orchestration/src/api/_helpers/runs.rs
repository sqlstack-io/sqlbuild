//! Worker reports are bound to the authenticated run and claim, never a caller-supplied actor.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{HeartbeatResponse, MutationResponse};
use crate::credentials::models::{CredentialScope, TokenIdentity};
use crate::runs::main::change_run::change_run;
use crate::runs::main::heartbeat_run::heartbeat_run;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/runs/{run_id}/events", params(("run_id" = Uuid, Path)),
    request_body = RunChangeRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Run not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn report(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(run_id): Path<Uuid>,
    Json(request): Json<RunChangeRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    let claim_id = require_worker(&identity, run_id)?;
    if request.run_id != run_id || request.claim_id != Some(claim_id) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "report does not match the authenticated run and claim".into(),
        });
    }
    let applied = change_run(&store, RunAuthority::Worker { run_id, claim_id }, &request).await?;
    Ok(Json(MutationResponse { applied }))
}

#[utoipa::path(post, path = "/api/runs/{run_id}/heartbeat", params(("run_id" = Uuid, Path)),
    responses((status = 200, body = HeartbeatResponse), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Run not authorized")), security(("bearer" = [])))]
pub(crate) async fn heartbeat(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<HeartbeatResponse>, ApiError> {
    let claim_id = require_worker(&identity, run_id)?;
    Ok(Json(HeartbeatResponse {
        cancel_requested: heartbeat_run(&store, (run_id, claim_id)).await?,
    }))
}

fn require_worker(identity: &TokenIdentity, run_id: Uuid) -> Result<Uuid, ApiError> {
    if let CredentialScope::Run {
        run_id: permitted,
        claim_id,
    } = identity.scope
        && permitted == run_id
    {
        return Ok(claim_id);
    }
    Err(ApiError {
        status: StatusCode::FORBIDDEN,
        message: "credential does not authorize this run".into(),
    })
}
