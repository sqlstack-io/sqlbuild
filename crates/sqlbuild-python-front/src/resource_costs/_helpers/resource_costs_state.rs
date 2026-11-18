//! DDL, appends, latest reads and pruning of the display-only resource costs table.

use crate::resource_costs::models::CostRunIdentity;
use chrono::DateTime;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyInt;
use sqlbuild_core::constants::RESOURCE_COSTS_TABLE_NAME;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_core::cost_report::models::CostSummary;
use sqlbuild_state::events::main::resource_cost_observation_id::resource_cost_observation_id;
use sqlbuild_state::resource_costs::constants::{
    NULLABLE_RESOURCE_COST_COLUMNS, RESOURCE_COST_COLUMN_TYPES, TOTAL_RESOURCE_TYPE,
};
use sqlbuild_state::resource_costs::main::prune_resource_costs_sql::prune_resource_costs_sql;
use sqlbuild_state::resource_costs::models::{ResourceCostKey, RunCostAttempt};

use crate::execution_history::main::parsed_iso::parsed_iso;
use crate::execution_history::main::utc::utc;
use crate::native_host::main::fixed_state_table_sql::fixed_state_table_sql;
use crate::native_host::main::integer_literal::integer_literal;
use crate::native_host::main::literal::literal;
use crate::native_host::main::optional_text::optional_text;
use crate::native_host::main::state_table_name::state_table_name;
use crate::native_host::models::Location;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Rate source of a price the project configured, the only one shown as USD.
const CONFIGURED_RATE_SOURCE: &str = "configured";
/// The adapter method that marks the resource costs table as supported.
pub(crate) const CREATE_TABLE_METHOD: &str = "render_create_resource_costs_table_sql";

/// Create-if-missing DDL for the resource costs table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
pub(crate) fn build_resource_costs_create_table_sql(
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
pub(crate) fn build_resource_costs_prune_sql(
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

pub(crate) fn table(adapter: &Bound<'_, PyAny>, location: &Location) -> PyResult<Option<String>> {
    let keywords = location.keywords(adapter.py());
    state_table_name(
        &native_field(adapter, "render_qualified_name")?,
        (&keywords[0].1, &keywords[1].1),
        RESOURCE_COSTS_TABLE_NAME,
    )
}

struct ResourceRow<'a> {
    resource_type: &'a str,
    resource_name: &'a str,
    warehouse_name: &'a str,
    warehouse_size: &'a str,
    warehouse_type: Option<&'a str>,
    cluster_number: Option<i64>,
    query_count: i64,
    attributed_seconds: &'a str,
    bytes_scanned: i64,
    estimated_credits: &'a str,
    estimated_usd: Option<String>,
}

/// The run row and one row per resource and warehouse of one collection attempt.
pub(crate) fn observation_cells(
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
        resource_type: TOTAL_RESOURCE_TYPE,
        resource_name: run.run_id,
        warehouse_name: "",
        warehouse_size: "",
        warehouse_type: None,
        cluster_number: None,
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
        warehouse_name: &resource.warehouse_name,
        warehouse_size: &resource.warehouse_size,
        warehouse_type: resource.warehouse_type.as_deref(),
        cluster_number: resource.cluster_number,
        query_count: resource.query_count,
        attributed_seconds: &resource.attributed_seconds,
        bytes_scanned: resource.bytes_scanned,
        estimated_credits: &resource.estimated_compute_credits,
        estimated_usd: usd(&resource.estimated_usd),
    }));
    rows.iter()
        .map(|row| {
            let observation_id: String = resource_cost_observation_id(&ResourceCostKey {
                run_id: run.run_id,
                attempt,
                resource_type: row.resource_type,
                resource_name: row.resource_name,
                warehouse_name: row.warehouse_name,
                warehouse_size: row.warehouse_size,
                warehouse_type: row.warehouse_type,
                cluster_number: row.cluster_number,
            })
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
            Ok(vec![
                literal(py, Some(&observation_id))?,
                literal(py, Some(run.run_id))?,
                literal(py, run.orchestration_run_id)?,
                integer_literal(py, Some(attempt))?,
                literal(py, Some(row.resource_type))?,
                literal(py, Some(row.resource_name))?,
                literal(py, Some(row.warehouse_name))?,
                literal(py, Some(row.warehouse_size))?,
                literal(py, row.warehouse_type)?,
                integer_literal(py, row.cluster_number)?,
                integer_literal(py, Some(row.query_count))?,
                literal(py, Some(row.attributed_seconds))?,
                integer_literal(py, Some(row.bytes_scanned))?,
                literal(py, Some(row.estimated_credits))?,
                literal(py, row.estimated_usd.as_deref())?,
                literal(py, Some(&summary.status))?,
                integer_literal(py, Some(summary.expected_statement_count))?,
                integer_literal(py, Some(summary.observed_statement_count))?,
                literal(py, Some(&limitations))?,
                literal(py, run.target_name)?,
                literal(py, Some(run.started_at))?,
                literal(py, Some(run.completed_at))?,
                observed_at.to_owned(),
            ])
        })
        .collect()
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

/// A stored run time as RFC 3339 UTC; the table is written in UTC, so a naive value is UTC.
fn utc_text(cell: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = cell.py();
    let value: Bound<'_, PyAny> = match cell.extract::<String>() {
        Ok(text) => parsed_iso(py, text.trim())?
            .ok_or_else(|| PyValueError::new_err(format!("unreadable run time {text:?}")))?,
        Err(_) => cell.clone(),
    };
    let aware: Bound<'_, PyAny> = if native_field(&value, "tzinfo")?.is_none() {
        call_with(&native_field(&value, "replace")?, &[("tzinfo", utc(py)?)])?
    } else {
        value.call_method1("astimezone", (utc(py)?,))?
    };
    aware.call_method0("isoformat")?.extract()
}

pub(crate) fn run_attempt(row: &Bound<'_, PyAny>) -> PyResult<RunCostAttempt> {
    let cell = |index: usize| row.get_item(index);
    let text = |index: usize| -> PyResult<String> {
        Ok(optional_text(&cell(index)?)?.unwrap_or_default())
    };
    let limitations: Vec<String> = serde_json::from_str(&text(7)?)
        .map_err(|error| PyValueError::new_err(format!("unreadable limitations: {error}")))?;
    let run_started_at: String = utc_text(&cell(9)?)?;
    let epoch = |text: &str| -> PyResult<i64> {
        Ok(DateTime::parse_from_rfc3339(text)
            .map_err(|error| PyValueError::new_err(error.to_string()))?
            .timestamp())
    };
    let started_epoch: i64 = epoch(&run_started_at)?;
    let run_completed_at: String = utc_text(&cell(10)?)?;
    let completed_epoch: i64 = epoch(&run_completed_at)?;
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
        run_completed_at,
        started_epoch,
        completed_epoch,
    })
}
