//! Worker reports are bound to the authenticated run and claim, never a caller-supplied actor.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use sqlbuild_recall::records::models::OutputChunk;
use sqlbuild_recall::store::models::InsertOutcome;
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{HeartbeatResponse, MutationResponse};
use crate::credentials::models::{CredentialScope, TokenIdentity};
use crate::runs::main::change_run::change_run;
use crate::runs::main::heartbeat_run::heartbeat_run;
use crate::runs::main::publish_run_output::publish_run_output;
use crate::runs::main::publish_run_recall::publish_run_recall;
use crate::runs::main::run_execution::run_execution;
use crate::runs::models::{
    OutputPublication, RecallBatch, RunAuthority, RunChangeRequest, RunExecution,
};
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/runs/{run_id}/execution", params(("run_id" = Uuid, Path)),
    responses((status = 200, body = RunExecution), (status = 401, description = "Inactive claim or invalid credential"),
        (status = 403, description = "Run not authorized")), security(("bearer" = [])))]
pub(crate) async fn execution(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunExecution>, ApiError> {
    let claim_id = require_worker(&identity, run_id)?;
    let execution = run_execution(&store, (run_id, claim_id))
        .await?
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "run claim is no longer active".into(),
        })?;
    Ok(Json(execution))
}

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

#[utoipa::path(post, path = "/api/runs/{run_id}/output", params(("run_id" = Uuid, Path)),
    request_body = Vec<OutputChunk>,
    responses((status = 200, body = OutputPublication), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Run not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn output(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(run_id): Path<Uuid>,
    Json(chunks): Json<Vec<OutputChunk>>,
) -> Result<Json<OutputPublication>, ApiError> {
    let claim_id = require_worker(&identity, run_id)?;
    Ok(Json(
        publish_run_output(&store, (run_id, claim_id), &chunks).await?,
    ))
}

#[utoipa::path(post, path = "/api/runs/{run_id}/recall", params(("run_id" = Uuid, Path)),
    request_body = RecallBatch,
    responses((status = 200, body = InsertOutcome), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Run not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn recall(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(run_id): Path<Uuid>,
    Json(batch): Json<RecallBatch>,
) -> Result<Json<InsertOutcome>, ApiError> {
    let claim_id = require_worker(&identity, run_id)?;
    Ok(Json(
        publish_run_recall(&store, (run_id, claim_id), &batch).await?,
    ))
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
