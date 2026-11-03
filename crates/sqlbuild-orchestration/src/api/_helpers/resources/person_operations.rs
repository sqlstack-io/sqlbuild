//! Person operations use person sessions, never the machine-resource bearer router.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::errors::ApiError;
use crate::api::models::{MutationResponse, ReadQuery};
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::main::change_person_trigger::change_person_trigger;
use crate::declarations::models::TriggerChangeRequest;
use crate::runs::main::cancel_person_run::cancel_person_run;
use crate::runs::main::list_person_runs::list_person_runs;
use crate::runs::main::request_person_job_run::request_person_job_run;
use crate::runs::main::request_rerun::request_rerun;
use crate::runs::models::{CancelRunRequest, JobRunRequest, RerunRequest, RunRecord};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::read_person_catalog::read_person_catalog;
use crate::warehouse_metadata::main::read_person_fingerprint::read_person_fingerprint;
use crate::warehouse_metadata::main::read_person_node_status::read_person_node_status;
use crate::warehouse_metadata::main::read_person_statistics::read_person_statistics;
use crate::warehouse_metadata::models::{
    CatalogPage, CatalogQuery, FingerprintObservation, FingerprintQuery, NodeStatus,
    NodeStatusQuery, StatisticsPage, StatisticsQuery, StatisticsRetention,
};

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/statistics", operation_id = "readPersonStatistics",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("database" = Option<String>, Query, description = "Exact database; omitted matches SQL NULL"),
        ("schema" = String, Query), ("name" = String, Query),
        ("after_observed_at" = Option<chrono::DateTime<chrono::Utc>>, Query),
        ("after_event_id" = Option<Uuid>, Query), ("limit" = Option<u32>, Query)),
    responses((status = 200, body = StatisticsPage), (status = 400, description = "Invalid relation or cursor"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn statistics(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(default_role), Extension(retention)): (
        Extension<DefaultRole>,
        Extension<StatisticsRetention>,
    ),
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<StatisticsQuery>),
    headers: HeaderMap,
) -> Result<Json<StatisticsPage>, ApiError> {
    Ok(Json(
        read_person_statistics(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, &options),
            (default_role, retention),
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/api/control/runs", operation_id = "requestJobRun",
    params(("x-sqb-csrf" = String, Header)), request_body = JobRunRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request_job(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    headers: HeaderMap,
    Json(request): Json<JobRunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_person_job_run(
            &store,
            (&context, session_credentials(&headers)),
            &request,
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/runs/cancel", operation_id = "cancelPersonRun",
    params(("x-sqb-csrf" = String, Header)), request_body = CancelRunRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn cancel_run(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    headers: HeaderMap,
    Json(request): Json<CancelRunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: cancel_person_run(&store, (&context, session_credentials(&headers)), &request)
            .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/runs/rerun", operation_id = "rerunPersonRun",
    params(("x-sqb-csrf" = String, Header)), request_body = RerunRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn rerun(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    headers: HeaderMap,
    Json(request): Json<RerunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_rerun(
            &store,
            (&context, Some(session_credentials(&headers))),
            &request,
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/triggers", operation_id = "changeTrigger",
    params(("x-sqb-csrf" = String, Header)), request_body = TriggerChangeRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn change_trigger(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    headers: HeaderMap,
    Json(request): Json<TriggerChangeRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: change_person_trigger(&store, (&context, session_credentials(&headers)), &request)
            .await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs", operation_id = "listPersonRuns",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("limit" = Option<u32>, Query)),
    responses((status = 200, body = Vec<RunRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn recent_runs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<ReadQuery>),
    headers: HeaderMap,
) -> Result<Json<Vec<RunRecord>>, ApiError> {
    Ok(Json(
        list_person_runs(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, options.limit.unwrap_or(100)),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/catalog", operation_id = "readPersonCatalog",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("database" = Option<String>, Query, description = "Exact database; omitted matches SQL NULL"),
        ("schema" = String, Query, description = "Exact physical schema"),
        ("after_observed_at" = Option<chrono::DateTime<chrono::Utc>>, Query),
        ("after_event_id" = Option<Uuid>, Query),
        ("limit" = Option<u32>, Query, description = "Page size; defaults to 100, clamped to 1..100")),
    responses((status = 200, body = CatalogPage), (status = 400, description = "Invalid namespace or cursor"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn catalog(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<CatalogQuery>),
    headers: HeaderMap,
) -> Result<Json<CatalogPage>, ApiError> {
    Ok(Json(
        read_person_catalog(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/fingerprint", operation_id = "readPersonFingerprint",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("node_type" = String, Query), ("node_name" = String, Query),
        ("database" = Option<String>, Query, description = "Exact destination database; omitted matches SQL NULL"),
        ("schema" = Option<String>, Query, description = "Exact destination schema; omitted matches SQL NULL"),
        ("name" = Option<String>, Query, description = "Exact destination name; omitted matches SQL NULL")),
    responses((status = 200, body = Option<FingerprintObservation>), (status = 400, description = "Invalid node"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn fingerprint(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<FingerprintQuery>),
    headers: HeaderMap,
) -> Result<Json<Option<FingerprintObservation>>, ApiError> {
    Ok(Json(
        read_person_fingerprint(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/node-status", operation_id = "readPersonNodeStatus",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("node_key" = String, Query, description = "One exact published node key")),
    responses((status = 200, body = Option<NodeStatus>), (status = 400, description = "Invalid node key"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn node_status(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<NodeStatusQuery>),
    headers: HeaderMap,
) -> Result<Json<Option<NodeStatus>>, ApiError> {
    Ok(Json(
        read_person_node_status(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}
