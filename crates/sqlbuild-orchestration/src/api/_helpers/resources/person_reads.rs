//! Person reads of run nodes and their errors, node run history, catalog nodes, jobs and one job's declaration, triggers, ticks and publications; refusal precedes every lookup, so unknown objects look forbidden.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{ReadQuery, RunEventsQuery, RunNodePath, StatementSqlQuery};
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::declarations::main::list_person_jobs::list_person_jobs;
use crate::declarations::main::list_person_triggers::list_person_triggers;
use crate::declarations::main::read_person_job::read_person_job;
use crate::declarations::models::{JobDetailRecord, JobRecord, PersonTriggerRecord};
use crate::platform::main::read_person_location_health::read_person_location_health;
use crate::platform::main::read_person_problems::read_person_problems;
use crate::platform::models::{LocationHealthObservation, PlatformProblems};
use crate::runs::main::list_person_run_nodes::list_person_run_nodes;
use crate::runs::models::{
    NodeRunRecord, RunEventRecord, RunNodeFailure, RunNodeRecord, RunOutput, RunStatementRecord,
    RunStatementSql,
};
use crate::scheduler::main::list_person_ticks::list_person_ticks;
use crate::scheduler::models::TickRecord;
use crate::snapshots::main::list_person_snapshots::list_person_snapshots;
use crate::snapshots::models::PublishedSnapshot;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_person_node_runs::read_person_node_runs;
use crate::warehouse_history::main::read_person_run_events::read_person_run_events;
use crate::warehouse_history::main::read_person_run_node_error::read_person_run_node_error;
use crate::warehouse_history::main::read_person_run_output::read_person_run_output;
use crate::warehouse_history::main::read_person_run_statement_sql::read_person_run_statement_sql;
use crate::warehouse_history::main::read_person_run_statements::read_person_run_statements;
use crate::warehouse_metadata::main::list_person_catalog_nodes::list_person_catalog_nodes;
use crate::warehouse_metadata::models::CatalogNodeRecord;

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes", operation_id = "listPersonRunNodes",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<RunNodeRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_nodes(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunNodeRecord>>, ApiError> {
    list_person_run_nodes(
        &store,
        (&context, principal),
        (location_id, &target, run_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/nodes", operation_id = "listPersonCatalogNodes",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<CatalogNodeRecord>, description = "Every node of the current snapshot; empty before the first publication"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn nodes(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<CatalogNodeRecord>>, ApiError> {
    Ok(Json(
        list_person_catalog_nodes(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/runs", operation_id = "listPersonNodeRuns",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("node_key" = String, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent runs; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<NodeRunRecord>, description = "Runs that built the node, newest first"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn node_runs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, node_key)), Query(options)): (
        Path<(Uuid, String, String)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<NodeRunRecord>>, ApiError> {
    Ok(Json(
        read_person_node_runs(
            &store,
            (&context, principal),
            (location_id, &target, &node_key),
            (options.limit.unwrap_or(100), default_role),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/events", operation_id = "listPersonRunEvents",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path),
        ("after" = Option<i64>, Query, description = "Return events after this storage cursor"),
        ("limit" = Option<u32>, Query, description = "Maximum events; defaults to 1000, capped at 5000")),
    responses((status = 200, body = Vec<RunEventRecord>, description = "Event metadata oldest first, without payloads"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_events(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<RunEventsQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunEventRecord>>, ApiError> {
    read_person_run_events(
        &store,
        (&context, principal),
        (
            (location_id, &target, run_id),
            (options.after.unwrap_or(0), options.limit.unwrap_or(1000)),
        ),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements", operation_id = "listPersonRunStatements",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum statements; defaults to 1000, capped at 5000")),
    responses((status = 200, body = Vec<RunStatementRecord>, description = "Statement metadata in start order, without SQL"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_statements(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunStatementRecord>>, ApiError> {
    read_person_run_statements(
        &store,
        (&context, principal),
        (
            (location_id, &target, run_id),
            options.limit.unwrap_or(1000),
        ),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements/sql", operation_id = "readPersonRunStatementSql",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path),
        ("statement_id" = String, Query, description = "One statement of this run")),
    responses((status = 200, body = RunStatementSql), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.sql on every node the run selected"),
        (status = 404, description = "Run or statement not found")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_statement_sql(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<StatementSqlQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<RunStatementSql>, ApiError> {
    read_person_run_statement_sql(
        &store,
        (&context, principal),
        ((location_id, &target, run_id), &options.statement_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("statement not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes/{node_key}/error", operation_id = "readPersonRunNodeError",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path), ("node_key" = String, Path)),
    responses((status = 200, body = RunNodeFailure), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on the node"),
        (status = 404, description = "Node not found in this run")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_node_error(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path(path): Path<RunNodePath>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<RunNodeFailure>, ApiError> {
    read_person_run_node_error(
        &store,
        (&context, principal),
        (path.location_id, &path.target),
        (path.run_id, &path.node_key),
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("node not found in this run"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output", operation_id = "readPersonRunOutput",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = RunOutput), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on every node the run selected"),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_output(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<RunOutput>, ApiError> {
    read_person_run_output(
        &store,
        (&context, principal),
        (location_id, &target, run_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/jobs/{job_key}", operation_id = "readPersonJob",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("job_key" = String, Path)),
    responses((status = 200, body = JobDetailRecord), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view on every current node of the target"),
        (status = 404, description = "No published job by that name")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn job(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, job_key)): Path<(Uuid, String, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<JobDetailRecord>, ApiError> {
    read_person_job(
        &store,
        (&context, principal),
        (location_id, &target, &job_key),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("job not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/jobs", operation_id = "listPersonJobs",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<JobRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn jobs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<JobRecord>>, ApiError> {
    Ok(Json(
        list_person_jobs(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers", operation_id = "listPersonTriggers",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<PersonTriggerRecord>, description = "Published triggers with their newest tick"), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn triggers(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<PersonTriggerRecord>>, ApiError> {
    Ok(Json(
        list_person_triggers(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks", operation_id = "listPersonTicks",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent ticks; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<TickRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Trigger not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn ticks(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, trigger_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<TickRecord>>, ApiError> {
    list_person_ticks(
        &store,
        (&context, principal),
        (
            (location_id, &target),
            (trigger_id, options.limit.unwrap_or(100)),
        ),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("trigger not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/snapshots", operation_id = "listPersonSnapshots",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent publications; defaults to 20, capped at 100")),
    responses((status = 200, body = Vec<PublishedSnapshot>, description = "Publications newest first, each with its changed nodes and roles"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn snapshots(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<ReadQuery>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<PublishedSnapshot>>, ApiError> {
    Ok(Json(
        list_person_snapshots(
            &store,
            (&context, principal),
            ((location_id, &target), options.limit.unwrap_or(20)),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/health", operation_id = "readPersonLocationHealth",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<LocationHealthObservation>, description = "This target's newest health observations and instance check-ins"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn health(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<LocationHealthObservation>>, ApiError> {
    Ok(Json(
        read_person_location_health(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/platform/problems", operation_id = "readPersonPlatformProblems",
    responses((status = 200, body = PlatformProblems, description = "Live scheduler state, and broken locations and late ticks of targets the person lists"),
        (status = 401, description = "Session rejected")), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn problems(
    State(store): State<ControlStore>,
    Extension(default_role): Extension<DefaultRole>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PlatformProblems>, ApiError> {
    Ok(Json(
        read_person_problems(&store, principal, default_role).await?,
    ))
}

fn not_found(message: &str) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: message.into(),
        decision_id: None,
    }
}
