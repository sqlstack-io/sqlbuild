//! DDL, appends, latest reads and pruning of the display-only resource costs table.

use chrono::DateTime;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::RESOURCE_COSTS_TABLE_NAME;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_core::cost_report::models::CostSummary;
use sqlbuild_state::consumption::constants::INTEGER_STATE_VALUE_TYPE;
use sqlbuild_state::resource_costs::constants::{
    NULLABLE_RESOURCE_COST_COLUMNS, RESOURCE_COST_COLUMN_TYPES, RUN_RESOURCE_TYPE,
};
use sqlbuild_state::resource_costs::main::latest_run_costs_sql::latest_run_costs_sql;
use sqlbuild_state::resource_costs::main::prune_resource_costs_sql::prune_resource_costs_sql;
use sqlbuild_state::resource_costs::main::resource_cost_observation_id::resource_cost_observation_id;
use sqlbuild_state::resource_costs::models::{ResourceCostKey, RunCostAttempt};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::native_host::consumption_state::{
    Location, ROWS_PER_STATEMENT, execute, fixed_state_table_sql, literal, optional_text,
    state_table_name,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Rate source of a price the project configured, the only one shown as USD.
const CONFIGURED_RATE_SOURCE: &str = "configured";

/// Create-if-missing DDL for the resource costs table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
fn build_resource_costs_create_table_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    fixed_state_table_sql(
        (database, schema),
        (render_qualified_name, render_framework_type),
        (
            RESOURCE_COSTS_TABLE_NAME,
            &RESOURCE_COST_COLUMN_TYPES,
            &NULLABLE_RESOURCE_COST_COLUMNS,
        ),
    )
}

/// SQL deleting observations beyond the newest `retain_versions` per run, resource and warehouse.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
fn build_resource_costs_prune_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    retain_versions: i64,
) -> PyResult<String> {
    Ok(state_table_name(
        render_qualified_name,
        (database, schema),
        RESOURCE_COSTS_TABLE_NAME,
    )?
    .map(|table| prune_resource_costs_sql(&table, retain_versions))
    .unwrap_or_default())
}

/// The run a collection belongs to, as the run row records it.
pub(crate) struct CostRunIdentity<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) orchestration_run_id: Option<&'a str>,
    pub(crate) target_name: Option<&'a str>,
    pub(crate) started_at: &'a str,
    pub(crate) completed_at: &'a str,
}

fn integer(py: Python<'_>, value: Option<i64>) -> PyResult<String> {
    let value: Bound<'_, PyAny> = value.map_or_else(
        || py.None().into_bound(py),
        |value| PyInt::new(py, value).into_any(),
    );
    render_state_sql_literal(py, &value, INTEGER_STATE_VALUE_TYPE, None)
}

fn table(adapter: &Bound<'_, PyAny>, location: &Location) -> PyResult<Option<String>> {
    let keywords = location.keywords(adapter.py());
    state_table_name(
        &native_field(adapter, "render_qualified_name")?,
        (&keywords[0].1, &keywords[1].1),
        RESOURCE_COSTS_TABLE_NAME,
    )
}

/// The run row and one row per resource and warehouse of one collection attempt.
fn observation_cells(
    py: Python<'_>,
    (run, attempt): (&CostRunIdentity<'_>, i64),
    summary: &CostSummary,
    observed_at: &str,
) -> PyResult<Vec<Vec<String>>> {
    let usd =
        |value: &str| (summary.rate_source == CONFIGURED_RATE_SOURCE).then(|| value.to_owned());
    let limitations: String = serde_json::to_string(&summary.limitations)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let run_row = ResourceRow {
        resource_type: RUN_RESOURCE_TYPE,
        resource_name: run.run_id,
        warehouse: ("", "", None, None),
        query_count: summary.query_count,
        attributed_seconds: &summary.attributed_seconds,
        bytes_scanned: summary.bytes_scanned,
        estimated_credits: &summary.estimated_compute_credits,
        estimated_usd: usd(&summary.estimated_usd),
    };
    let mut rows: Vec<ResourceRow<'_>> = vec![run_row];
    rows.extend(summary.resources.iter().map(|resource| ResourceRow {
        resource_type: &resource.resource_type,
        resource_name: &resource.resource_name,
        warehouse: (
            &resource.warehouse_name,
            &resource.warehouse_size,
            resource.warehouse_type.as_deref(),
            resource.cluster_number,
        ),
        query_count: resource.query_count,
        attributed_seconds: &resource.attributed_seconds,
        bytes_scanned: resource.bytes_scanned,
        estimated_credits: &resource.estimated_compute_credits,
        estimated_usd: usd(&resource.estimated_usd),
    }));
    rows.iter()
        .map(|row| {
            let (warehouse_name, warehouse_size, warehouse_type, cluster_number) = row.warehouse;
            let observation_id: String = resource_cost_observation_id(&ResourceCostKey {
                run_id: run.run_id,
                attempt,
                resource_type: row.resource_type,
                resource_name: row.resource_name,
                warehouse_name,
                warehouse_size,
                warehouse_type,
                cluster_number,
            })
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
            Ok(vec![
                literal(py, Some(&observation_id))?,
                literal(py, Some(run.run_id))?,
                literal(py, run.orchestration_run_id)?,
                integer(py, Some(attempt))?,
                literal(py, Some(row.resource_type))?,
                literal(py, Some(row.resource_name))?,
                literal(py, Some(warehouse_name))?,
                literal(py, Some(warehouse_size))?,
                literal(py, warehouse_type)?,
                integer(py, cluster_number)?,
                integer(py, Some(row.query_count))?,
                literal(py, Some(row.attributed_seconds))?,
                integer(py, Some(row.bytes_scanned))?,
                literal(py, Some(row.estimated_credits))?,
                literal(py, row.estimated_usd.as_deref())?,
                literal(py, Some(&summary.status))?,
                integer(py, Some(summary.expected_statement_count))?,
                integer(py, Some(summary.observed_statement_count))?,
                literal(py, Some(&limitations))?,
                literal(py, run.target_name)?,
                literal(py, Some(run.started_at))?,
                literal(py, Some(run.completed_at))?,
                observed_at.to_owned(),
            ])
        })
        .collect()
}

struct ResourceRow<'a> {
    resource_type: &'a str,
    resource_name: &'a str,
    /// Name, size, type and cluster number.
    warehouse: (&'a str, &'a str, Option<&'a str>, Option<i64>),
    query_count: i64,
    attributed_seconds: &'a str,
    bytes_scanned: i64,
    estimated_credits: &'a str,
    estimated_usd: Option<String>,
}

/// Create the schema and table in `location` if missing, then append one collection attempt.
pub(crate) fn append_resource_costs(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    (run, attempt): (&CostRunIdentity<'_>, i64),
    summary: &CostSummary,
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    let Some(table) = table(adapter, location)? else {
        return Ok(());
    };
    let keywords = location.keywords(py);
    for statement in
        call_with(&native_field(adapter, "render_create_schema")?, &keywords)?.try_iter()?
    {
        execute(adapter, connection, &statement?.extract::<String>()?)?;
    }
    let create: String = call_with(
        &native_field(adapter, "render_create_resource_costs_table_sql")?,
        &keywords,
    )?
    .extract()?;
    if !create.is_empty() {
        execute(adapter, connection, &create)?;
    }
    let observed_at: String =
        call_with(&native_field(adapter, "render_current_utc_timestamp")?, &[])?.extract()?;
    let columns: Vec<&str> = RESOURCE_COST_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    let cells: Vec<Vec<String>> = observation_cells(py, (run, attempt), summary, &observed_at)?;
    for chunk in cells.chunks(ROWS_PER_STATEMENT) {
        execute(
            adapter,
            connection,
            &values_insert_sql(&table, &columns, chunk),
        )?;
    }
    Ok(())
}

fn cell_integer(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    if value.is_instance_of::<PyInt>() {
        return value.extract();
    }
    optional_text(value)?
        .unwrap_or_default()
        .trim()
        .parse()
        .map_err(|_| PyValueError::new_err("resource cost integer cell is not an integer"))
}

fn run_attempt(row: &Bound<'_, PyAny>) -> PyResult<RunCostAttempt> {
    let cell = |index: usize| row.get_item(index);
    let text = |index: usize| -> PyResult<String> {
        Ok(optional_text(&cell(index)?)?.unwrap_or_default())
    };
    let limitations: Vec<String> = serde_json::from_str(&text(7)?).unwrap_or_default();
    let run_started_at: String = text(9)?;
    let started_epoch: i64 = DateTime::parse_from_rfc3339(run_started_at.trim())
        .map_err(|error| PyValueError::new_err(error.to_string()))?
        .timestamp();
    Ok(RunCostAttempt {
        run_id: text(0)?,
        orchestration_run_id: optional_text(&cell(1)?)?,
        attempt: cell_integer(&cell(2)?)?,
        coverage: CostCoverage {
            status: text(3)?,
            expected_statement_count: cell_integer(&cell(4)?)?,
            observed_statement_count: cell_integer(&cell(5)?)?,
            query_count: cell_integer(&cell(6)?)?,
            limitations,
        },
        target_name: optional_text(&cell(8)?)?,
        run_started_at,
        run_completed_at: text(10)?,
        started_epoch,
    })
}

/// The newest run row of each refreshable run in `location`; none when the table is missing.
/// A row that does not parse is skipped, so one bad row never blocks the others.
pub(crate) fn latest_run_costs(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<Vec<RunCostAttempt>> {
    let Some(table) = table(adapter, location)? else {
        return Ok(Vec::new());
    };
    let py: Python<'_> = adapter.py();
    let mut keywords = location.keywords(py);
    keywords.push(("connection", connection.clone()));
    keywords.push((
        "name",
        PyString::new(py, RESOURCE_COSTS_TABLE_NAME).into_any(),
    ));
    let exists: bool = call_with(&native_field(adapter, "get_columns")?, &keywords)?.is_truthy()?;
    if !exists {
        return Ok(Vec::new());
    }
    let result: Bound<'_, PyAny> = execute(adapter, connection, &latest_run_costs_sql(&table))?;
    Ok(call_with(&native_field(&result, "fetchall")?, &[])?
        .try_iter()?
        .filter_map(|row| row.and_then(|row| run_attempt(&row)).ok())
        .collect())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_resource_costs_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_resource_costs_prune_sql, module)?)
}
