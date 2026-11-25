//! Person reads of run nodes and their errors, node run history, catalog nodes, jobs and one job's declaration, triggers, ticks and publications; refusal precedes every lookup, so unknown objects look forbidden.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::_helpers::resources::cached_json::cached_json;
use crate::api::_helpers::resources::runs::no_output_store;
use crate::api::errors::ApiError;
use crate::api::models::BrowserOrigin;
use crate::api::models::{
    CostQuery, PlatformEventsQuery, ReadQuery, RunEventsQuery, RunNodePath, ServerFacts,
    StatementSqlQuery,
};
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::declarations::main::list_person_jobs::list_person_jobs;
use crate::declarations::main::list_person_trigger_changes::list_person_trigger_changes;
use crate::declarations::main::list_person_triggers::list_person_triggers;
use crate::declarations::main::read_person_job::read_person_job;
use crate::declarations::main::read_person_providers::read_person_providers;
use crate::declarations::models::{
    JobDetailRecord, JobRecord, PersonTriggerChange, PersonTriggerRecord, ProvidersRead,
};
use crate::health::main::failing_forwarding::failing_forwarding;
use crate::health::models::ServerHealth;
use crate::node_states::main::list_person_catalog_nodes::list_person_catalog_nodes;
use crate::node_states::main::read_person_node_states::read_person_node_states;
use crate::node_states::models::{NodeStateCache, NodeStatesRecord};
use crate::platform::main::health::read_person_location_health::read_person_location_health;
use crate::platform::main::person_reads::list_person_platform_events::list_person_platform_events;
use crate::platform::main::person_reads::read_person_problems::read_person_problems;
use crate::platform::models::{PersonLocationHealth, PersonPlatformEventPage, PlatformProblems};
use crate::run_output::main::download_person_output::download_person_output;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::main::search_person_output::search_person_output;
use crate::run_output::models::{
    OutputDownloadQuery, OutputIndexQuery, OutputPage, OutputPageQuery, OutputSearch,
    OutputSearchQuery, OutputStream, RunOutputIndex,
};
use crate::runs::main::listings::list_person_run_nodes::list_person_run_nodes;
use crate::runs::models::{
    NodeRunRecord, RunEventRecord, RunIgnoredAuditFailure, RunNodeFailure, RunNodeRecord,
    RunOutput, RunStatementRecord, RunStatementSql,
};
use crate::scheduler::main::list_person_ticks::list_person_ticks;
use crate::scheduler::models::TickRecord;
use crate::sensors::main::read_person_sensor::read_person_sensor;
use crate::sensors::main::read_person_sensor_events::read_person_sensor_events;
use crate::sensors::models::{PersonSensorState, SensorEventChoice, SensorTrialEvents};
use crate::snapshots::main::reads::list_person_snapshots::list_person_snapshots;
use crate::snapshots::models::PublishedSnapshot;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::checks::read_person_run_ignored_audits::read_person_run_ignored_audits;
use crate::warehouse_history::main::costs::read_person_cost_summary::read_person_cost_summary;
use crate::warehouse_history::main::costs::read_person_node_costs::read_person_node_costs;
use crate::warehouse_history::main::costs::read_person_run_cost::read_person_run_cost;
use crate::warehouse_history::main::runs::read_person_node_runs::read_person_node_runs;
use crate::warehouse_history::main::runs::read_person_run_events::read_person_run_events;
use crate::warehouse_history::main::runs::read_person_run_node_error::read_person_run_node_error;
use crate::warehouse_history::main::runs::read_person_run_output::read_person_run_output;
use crate::warehouse_history::main::runs::read_person_run_statement_sql::read_person_run_statement_sql;
use crate::warehouse_history::main::runs::read_person_run_statements::read_person_run_statements;
use crate::warehouse_history::models::{CostSummaryRecord, NodeCostRecord};
use crate::warehouse_metadata::models::{CatalogNodeRecord, WarehouseResourceCostRecord};
use futures_util::StreamExt;

/// Default and largest cost window, in days.
const COST_DAYS: (u32, u32) = (7, 90);

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
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("If-None-Match" = Option<String>, Header, description = "An ETag from an earlier response; 304 when the snapshot is unchanged")),
    responses((status = 200, body = Vec<CatalogNodeRecord>, description = "Every node of the current snapshot without its state, which node-states serves; empty before the first publication",
            headers(("ETag" = String, description = "\"<snapshot_id>\"; absent before the first publication"))),
        (status = 304, description = "Unchanged since the If-None-Match ETag", headers(("ETag" = String))),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn nodes(
    (Extension(cache), Extension(context)): (Extension<NodeStateCache>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), headers): (Path<(Uuid, String)>, HeaderMap),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Response, ApiError> {
    let listing = list_person_catalog_nodes(
        &cache,
        (&context, principal),
        (location_id, &target),
        default_role,
    )
    .await?;
    cached_json(listing, &headers)
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/node-states", operation_id = "readPersonNodeStates",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = NodeStatesRecord, description = "Every current node's state at the state version node_status events continue from"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn node_states(
    (Extension(cache), Extension(context)): (Extension<NodeStateCache>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Response, ApiError> {
    let states = read_person_node_states(
        &cache,
        (&context, principal),
        (location_id, &target),
        default_role,
    )
    .await?;
    cached_json(states, &HeaderMap::new())
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/ignored-audits", operation_id = "listPersonRunIgnoredAudits",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<RunIgnoredAuditFailure>, description = "The error-severity audit failures the run and its current group members let through, run by run"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_ignored_audits(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunIgnoredAuditFailure>>, ApiError> {
    read_person_run_ignored_audits(
        &store,
        (&context, principal),
        (location_id, &target, run_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/cost", operation_id = "readPersonRunCost",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<WarehouseResourceCostRecord>, description = "The latest estimated query cost observation of the run and of each resource and warehouse it used; empty when none was synced"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_cost(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<WarehouseResourceCostRecord>>, ApiError> {
    read_person_run_cost(
        &store,
        (&context, principal),
        (location_id, &target, run_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

fn cost_days(query: &CostQuery) -> u32 {
    query.days.unwrap_or(COST_DAYS.0).clamp(1, COST_DAYS.1)
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/cost", operation_id = "readPersonCostSummary",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("days" = Option<u32>, Query, description = "Runs that started in the last this many days; defaults to 7, capped at 90")),
    responses((status = 200, body = CostSummaryRecord, description = "Estimated query cost by job, warehouse size and node, most expensive first"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn cost_summary(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<CostQuery>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<CostSummaryRecord>, ApiError> {
    Ok(Json(
        read_person_cost_summary(
            &store,
            (&context, principal),
            (location_id, &target),
            (cost_days(&options), default_role),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/cost", operation_id = "listPersonNodeCosts",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("node_key" = String, Path),
        ("days" = Option<u32>, Query, description = "Builds that started in the last this many days; defaults to 7, capped at 90"),
        ("limit" = Option<u32>, Query, description = "Maximum builds; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<NodeCostRecord>, description = "The node's estimated query cost in each build, newest first"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn node_costs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, node_key)), Query(options)): (
        Path<(Uuid, String, String)>,
        Query<CostQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<NodeCostRecord>>, ApiError> {
    Ok(Json(
        read_person_node_costs(
            &store,
            (&context, principal),
            (location_id, &target, &node_key),
            (
                cost_days(&options),
                options.limit.unwrap_or(100).min(1000),
                default_role,
            ),
        )
        .await?,
    ))
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/index", operation_id = "readPersonRunOutputIndex",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path), OutputIndexQuery),
    responses((status = 200, body = RunOutputIndex), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on every node the run selected"),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_output_index(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<OutputIndexQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<RunOutputIndex>, ApiError> {
    read_person_output_index(
        &store,
        (
            (&context, principal),
            (location_id, &target, run_id),
            default_role,
        ),
        query.node.as_deref(),
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/lines", operation_id = "readPersonRunOutputLines",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path), OutputPageQuery),
    responses((status = 200, body = OutputPage), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on every node the run selected"),
        (status = 404, description = "Run not found for a person who may view the whole target"),
        (status = 503, body = crate::api::models::ApiErrorBody, description = "The server keeps no run output")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_output_lines(
    (State(store), Extension(context), Extension(facts)): (
        State<ControlStore>,
        Extension<AuditContext>,
        Extension<ServerFacts>,
    ),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<OutputPageQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<OutputPage>, ApiError> {
    let output = facts.output.ok_or_else(no_output_store)?;
    read_person_output_page(
        &store,
        &output,
        (
            (&context, principal),
            (location_id, &target, run_id),
            default_role,
        ),
        &query,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/search", operation_id = "searchPersonRunOutput",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path), OutputSearchQuery),
    responses((status = 200, body = OutputSearch), (status = 400, body = crate::api::models::ApiErrorBody, description = "Empty search text"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on every node the run selected"),
        (status = 404, description = "Run not found for a person who may view the whole target"),
        (status = 503, body = crate::api::models::ApiErrorBody, description = "The server keeps no run output")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_output_search(
    (State(store), Extension(context), Extension(facts)): (
        State<ControlStore>,
        Extension<AuditContext>,
        Extension<ServerFacts>,
    ),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<OutputSearchQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<OutputSearch>, ApiError> {
    let output = facts.output.ok_or_else(no_output_store)?;
    search_person_output(
        &store,
        &output,
        (
            (&context, principal),
            (location_id, &target, run_id),
            default_role,
        ),
        &query,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/download", operation_id = "downloadPersonRunOutput",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path), OutputDownloadQuery),
    responses((status = 200, content_type = "text/plain", body = String, description = "The node's whole stream"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view.evidence on every node the run selected"),
        (status = 404, description = "Run not found for a person who may view the whole target"),
        (status = 503, body = crate::api::models::ApiErrorBody, description = "The server keeps no run output")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_output_download(
    (State(store), Extension(context), Extension(facts)): (
        State<ControlStore>,
        Extension<AuditContext>,
        Extension<ServerFacts>,
    ),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, run_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<OutputDownloadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Response, ApiError> {
    let output = facts.output.ok_or_else(no_output_store)?;
    let text = download_person_output(
        &store,
        &output,
        (
            (&context, principal),
            (location_id, &target, run_id),
            default_role,
        ),
        &query,
    )
    .await?
    .ok_or_else(|| not_found("run not found"))?;
    let file = format!(
        "attachment; filename=\"{run_id}-{}-{}.txt\"",
        query
            .node
            .replace(|character: char| !character.is_ascii_alphanumeric(), "_"),
        query.stream.map_or("output", OutputStream::as_str)
    );
    let mut response =
        axum::body::Body::from_stream(text.map(|chunk| chunk.map_err(std::io::Error::other)))
            .into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&file).map_err(|_| not_found("run not found"))?,
    );
    Ok(response)
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/jobs/{job_key}", operation_id = "readPersonJob",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("job_key" = String, Path)),
    responses((status = 200, body = JobDetailRecord), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody, description = "Needs view on every current node of the target"),
        (status = 404, description = "No published job by that name")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn job(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(default_role), Extension(facts)): (Extension<DefaultRole>, Extension<ServerFacts>),
    Path((location_id, target, job_key)): Path<(Uuid, String, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<JobDetailRecord>, ApiError> {
    read_person_job(
        &store,
        (&context, principal),
        (location_id, &target, &job_key),
        (default_role, &facts.pool_caps),
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
    (Extension(default_role), Extension(facts)): (Extension<DefaultRole>, Extension<ServerFacts>),
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<JobRecord>>, ApiError> {
    Ok(Json(
        list_person_jobs(
            &store,
            (&context, principal),
            (location_id, &target),
            (default_role, &facts.pool_caps),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/providers", operation_id = "readPersonProviders",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = ProvidersRead, description = "Declared providers without setting values"), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn providers(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<ProvidersRead>, ApiError> {
    Ok(Json(
        read_person_providers(
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/changes", operation_id = "listPersonTriggerChanges",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent changes; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<PersonTriggerChange>, description = "Operator changes, newest first"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Trigger not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn trigger_changes(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, trigger_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<PersonTriggerChange>>, ApiError> {
    list_person_trigger_changes(
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/sensor", operation_id = "readPersonSensorState",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent checks and run requests; defaults to 100, capped at 1000")),
    responses((status = 200, body = PersonSensorState, description = "The cursor, recent checks newest first and requested runs"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Sensor not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn sensor_state(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, trigger_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PersonSensorState>, ApiError> {
    read_person_sensor(
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
    .ok_or_else(|| not_found("sensor not found"))
}

/// How `sqb sensor try` chooses events: at most one of `latest`, `event` and `run`, else the newest `limit`.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct SensorEventsQuery {
    limit: Option<u32>,
    latest: Option<bool>,
    event: Option<String>,
    run: Option<Uuid>,
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/sensors/{name}/events", operation_id = "readPersonSensorEvents",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("name" = String, Path), SensorEventsQuery),
    responses((status = 200, body = SensorTrialEvents, description = "The published sensor and the chosen events, newest first; reading them changes nothing"),
        (status = 400, body = crate::api::models::ApiErrorBody, description = "More than one of latest, event and run"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 409, body = crate::api::models::ApiErrorBody, description = "No such sensor, event or matching run, and why")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn sensor_events(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(default_role), origin): (Extension<DefaultRole>, Option<Extension<BrowserOrigin>>),
    (Path((location_id, target, name)), Query(options)): (
        Path<(Uuid, String, String)>,
        Query<SensorEventsQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<SensorTrialEvents>, ApiError> {
    let choice = match (options.latest.unwrap_or(false), options.event, options.run) {
        (false, None, None) => SensorEventChoice::Recent {
            limit: options.limit.unwrap_or(10),
        },
        (true, None, None) => SensorEventChoice::Latest,
        (false, Some(event_id), None) => SensorEventChoice::Event { event_id },
        (false, None, Some(run_id)) => SensorEventChoice::Run { run_id },
        _ => {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "choose at most one of latest, event and run".into(),
                decision_id: None,
                cause: None,
            });
        }
    };
    let origin = origin.map(|Extension(BrowserOrigin(origin))| origin);
    read_person_sensor_events(
        &store,
        (&context, principal),
        ((location_id, &target), (&name, &choice)),
        (default_role, origin.as_deref()),
    )
    .await?
    .map(Json)
    .map_err(|message| ApiError {
        status: StatusCode::CONFLICT,
        message,
        decision_id: None,
        cause: None,
    })
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
    responses((status = 200, body = PersonLocationHealth, description = "This target's state and the newest health observations and instance check-ins it was computed from"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn health(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(default_role), Extension(facts)): (Extension<DefaultRole>, Extension<ServerFacts>),
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PersonLocationHealth>, ApiError> {
    Ok(Json(
        read_person_location_health(
            &store,
            (&context, principal),
            (location_id, &target),
            (default_role, &facts.pool_caps),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/platform/problems", operation_id = "readPersonPlatformProblems",
    responses((status = 200, body = PlatformProblems, description = "Live scheduler state, and broken locations and late ticks of targets the person lists"),
        (status = 401, description = "Session rejected")), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn problems(
    State(store): State<ControlStore>,
    (Extension(default_role), Extension(health)): (Extension<DefaultRole>, Extension<ServerHealth>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PlatformProblems>, ApiError> {
    Ok(Json(PlatformProblems {
        failing_forwarding: failing_forwarding(&health),
        ..read_person_problems(&store, principal, default_role).await?
    }))
}

#[utoipa::path(get, path = "/api/control/platform/events", operation_id = "listPersonPlatformEvents",
    params(("before" = Option<i64>, Query, description = "Continue after this storage ID of the previous page"),
        ("limit" = Option<u32>, Query, description = "Events to scan; defaults to 200, clamped to 1..500")),
    responses((status = 200, body = PersonPlatformEventPage, description = "One scan of stored platform events newest first, without heartbeats: server-wide ones, and those of targets the person lists"),
        (status = 401, description = "Session rejected")), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn platform_events(
    (State(store), Query(options)): (State<ControlStore>, Query<PlatformEventsQuery>),
    Extension(default_role): Extension<DefaultRole>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PersonPlatformEventPage>, ApiError> {
    Ok(Json(
        list_person_platform_events(
            &store,
            principal,
            (options.before, options.limit.unwrap_or(200)),
            default_role,
        )
        .await?,
    ))
}

fn not_found(message: &str) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: message.into(),
        decision_id: None,
        cause: None,
    }
}
