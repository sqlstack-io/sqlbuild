//! Person operations use person sessions, never the machine-resource bearer router.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::acknowledgements::main::acknowledge_person_run::acknowledge_person_run;
use crate::acknowledgements::main::list_person_acknowledgements::list_person_acknowledgements;
use crate::acknowledgements::models::{AcknowledgementRequest, RunAcknowledgement};
use crate::api::errors::ApiError;
use crate::api::models::{MutationResponse, RunsQuery};
use crate::credentials::models::AuthenticatedPerson;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::main::change_person_trigger::change_person_trigger;
use crate::declarations::models::TriggerChangeRequest;
use crate::runs::main::check_adhoc_run::check_adhoc_run;
use crate::runs::main::list_person_runs::list_person_runs;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::main::request_rerun::request_rerun;
use crate::runs::models::{
    AdhocCheck, AdhocCheckRequest, AdhocRunRequest, CancelRunRequest, JobRunRequest,
    PersonRunRequest, RerunRequest, RunRecord,
};
use crate::runs::types::{CommandPlanner, RunsListing};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::read_person_catalog::read_person_catalog;
use crate::warehouse_metadata::main::read_person_column_lineage::read_person_column_lineage;
use crate::warehouse_metadata::main::read_person_fingerprint::read_person_fingerprint;
use crate::warehouse_metadata::main::read_person_node_status::read_person_node_status;
use crate::warehouse_metadata::main::read_person_statistics::read_person_statistics;
use crate::warehouse_metadata::models::{
    CatalogPage, CatalogQuery, ColumnLineageRead, FingerprintObservation, FingerprintQuery,
    NodeStatus, NodeStatusQuery, StatisticsPage, StatisticsQuery, StatisticsRetention,
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
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<StatisticsPage>, ApiError> {
    Ok(Json(
        read_person_statistics(
            &store,
            (&context, principal),
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
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Json(request): Json<JobRunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_person_run(
            &store,
            (&context, principal),
            PersonRunRequest::Job(planner.as_ref(), &request),
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/runs/adhoc", operation_id = "requestAdhocRun",
    params(("x-sqb-csrf" = String, Header)), request_body = AdhocRunRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request_adhoc(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Json(request): Json<AdhocRunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_person_run(
            &store,
            (&context, principal),
            PersonRunRequest::Adhoc(planner.as_ref(), &request),
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/runs/adhoc/check", operation_id = "checkAdhocRun",
    params(("x-sqb-csrf" = String, Header)), request_body = AdhocCheckRequest,
    responses((status = 200, body = AdhocCheck), (status = 401, description = "Session rejected")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn check_adhoc(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Json(request): Json<AdhocCheckRequest>,
) -> Result<Json<AdhocCheck>, ApiError> {
    Ok(Json(
        check_adhoc_run(&store, (&context, principal), planner.as_ref(), &request).await?,
    ))
}

#[utoipa::path(post, path = "/api/control/runs/cancel", operation_id = "cancelPersonRun",
    params(("x-sqb-csrf" = String, Header)), request_body = CancelRunRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn cancel_run(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<CancelRunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_person_run(
            &store,
            (&context, principal),
            PersonRunRequest::Cancel(&request),
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/control/runs/rerun", operation_id = "rerunPersonRun",
    params(("x-sqb-csrf" = String, Header)), request_body = RerunRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn rerun(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<RerunRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_rerun(&store, (&context, Some(principal)), &request).await?,
    }))
}

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/acknowledgements", operation_id = "acknowledgeRun",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header)),
    request_body = AcknowledgementRequest,
    responses((status = 200, body = MutationResponse, description = "Whether the run's acknowledgement changed"),
        (status = 400, description = "The run is not in this location and target, or the event ID was reused"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn acknowledge(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Extension(default_role)): (
        Path<(Uuid, String)>,
        Extension<DefaultRole>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<AcknowledgementRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: acknowledge_person_run(
            &store,
            (&context, principal),
            (location_id, &target, &request),
            default_role,
        )
        .await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/acknowledgements", operation_id = "listRunAcknowledgements",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<RunAcknowledgement>, description = "Runs marked handled in the last week"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn acknowledgements(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunAcknowledgement>>, ApiError> {
    Ok(Json(
        list_person_acknowledgements(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/api/control/triggers", operation_id = "changeTrigger",
    params(("x-sqb-csrf" = String, Header)), request_body = TriggerChangeRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn change_trigger(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<TriggerChangeRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: change_person_trigger(&store, (&context, principal), &request).await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs", operation_id = "listPersonRuns",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("limit" = Option<u32>, Query, description = "Page size; defaults to 100, capped at 1000"),
        ("before_queued_at" = Option<chrono::DateTime<chrono::Utc>>, Query, description = "With before_run_id: continue after this run of the previous page"),
        ("before_run_id" = Option<Uuid>, Query),
        ("queued" = Option<bool>, Query, description = "List queued runs in claim order, each with its position and what it waits on")),
    responses((status = 200, body = Vec<RunRecord>, description = "Runs newest first, or the queue in claim order"),
        (status = 400, description = "Only one of the before_* pair was given, or a cursor with queued"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn recent_runs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<RunsQuery>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunRecord>>, ApiError> {
    let limit = options.limit.unwrap_or(100);
    let listing = match (
        options.queued,
        options.before_queued_at,
        options.before_run_id,
    ) {
        (false, Some(queued_at), Some(run_id)) => {
            RunsListing::Newest(limit, Some((queued_at, run_id)))
        }
        (false, None, None) => RunsListing::Newest(limit, None),
        (true, None, None) => RunsListing::Queue(limit),
        (true, ..) => {
            return Err(ControlError::Invalid("the queue is not paged".into()).into());
        }
        _ => {
            return Err(ControlError::Invalid(
                "before_queued_at and before_run_id go together".into(),
            )
            .into());
        }
    };
    Ok(Json(
        list_person_runs(
            &store,
            (&context, principal),
            (location_id, &target, listing),
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
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<CatalogPage>, ApiError> {
    Ok(Json(
        read_person_catalog(
            &store,
            (&context, principal),
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
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<FingerprintObservation>>, ApiError> {
    Ok(Json(
        read_person_fingerprint(
            &store,
            (&context, principal),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/column-lineage", operation_id = "readPersonColumnLineage",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("node_key" = String, Query, description = "The node whose upstream and downstream column lineage to return")),
    responses((status = 200, body = ColumnLineageRead), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn column_lineage(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<NodeStatusQuery>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<ColumnLineageRead>, ApiError> {
    Ok(Json(
        read_person_column_lineage(
            &store,
            (&context, principal),
            (location_id, &target, &options.node_key),
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
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<NodeStatus>>, ApiError> {
    Ok(Json(
        read_person_node_status(
            &store,
            (&context, principal),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}
