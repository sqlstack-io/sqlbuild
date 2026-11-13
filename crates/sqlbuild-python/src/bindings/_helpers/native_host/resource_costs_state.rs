//! DDL, appends, latest reads and pruning of the display-only resource costs table.

use chrono::DateTime;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyFloat, PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::RESOURCE_COSTS_TABLE_NAME;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_core::cost_report::models::CostSummary;
use sqlbuild_state::consumption::constants::INTEGER_STATE_VALUE_TYPE;
use sqlbuild_state::events::main::resource_cost_observation_id::resource_cost_observation_id;
use sqlbuild_state::resource_costs::constants::{
    NULLABLE_RESOURCE_COST_COLUMNS, RESOURCE_COST_COLUMN_TYPES, TOTAL_RESOURCE_TYPE,
};
use sqlbuild_state::resource_costs::main::latest_run_costs_sql::latest_run_costs_sql;
use sqlbuild_state::resource_costs::main::prune_resource_costs_sql::prune_resource_costs_sql;
use sqlbuild_state::resource_costs::models::{ResourceCostKey, RunCostAttempt};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::execution_history::conversion::{parsed_iso, utc};
use crate::bindings::_helpers::native_host::consumption_state::{
    Location, ROWS_PER_STATEMENT, execute, fixed_state_table_sql, literal, optional_text,
    state_table_name,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Rate source of a price the project configured, the only one shown as USD.
const CONFIGURED_RATE_SOURCE: &str = "configured";
/// The adapter method that marks the resource costs table as supported.
const CREATE_TABLE_METHOD: &str = "render_create_resource_costs_table_sql";
/// Collection budget of each re-read of an older run, which a build waits for.
const REFRESH_BUDGET_SECONDS: f64 = 20.0;

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
                integer(py, Some(attempt))?,
                literal(py, Some(row.resource_type))?,
                literal(py, Some(row.resource_name))?,
                literal(py, Some(row.warehouse_name))?,
                literal(py, Some(row.warehouse_size))?,
                literal(py, row.warehouse_type)?,
                integer(py, row.cluster_number)?,
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

fn run_attempt(row: &Bound<'_, PyAny>) -> PyResult<RunCostAttempt> {
    let cell = |index: usize| row.get_item(index);
    let text = |index: usize| -> PyResult<String> {
        Ok(optional_text(&cell(index)?)?.unwrap_or_default())
    };
    let limitations: Vec<String> = serde_json::from_str(&text(7)?)
        .map_err(|error| PyValueError::new_err(format!("unreadable limitations: {error}")))?;
    let run_started_at: String = utc_text(&cell(9)?)?;
    let started_epoch: i64 = DateTime::parse_from_rfc3339(&run_started_at)
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
        run_completed_at: utc_text(&cell(10)?)?,
        started_epoch,
    })
}

/// A target's resource costs table, open on one connection for one build's publication.
pub(crate) struct ResourceCostStore<'py> {
    finalization: Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
    connection: Bound<'py, PyAny>,
    location: Location,
    table: String,
}

impl<'py> ResourceCostStore<'py> {
    /// Connect when the adapter keeps resource costs and the target has a schema.
    pub(crate) fn open(finalization: &Bound<'py, PyAny>) -> PyResult<Option<Self>> {
        let adapter: Bound<'py, PyAny> = native_field(finalization, "adapter")?;
        if !adapter.hasattr(CREATE_TABLE_METHOD)? {
            return Ok(None);
        }
        let Some(schema) =
            native_field(finalization, "target_schema")?.extract::<Option<String>>()?
        else {
            return Ok(None);
        };
        let location = Location {
            database: native_field(finalization, "target_database")?.extract()?,
            schema,
        };
        let Some(table) = table(&adapter, &location)? else {
            return Ok(None);
        };
        let connection: Bound<'py, PyAny> = native_field(&adapter, "connect")?
            .call1((native_field(finalization, "connection_config")?,))?;
        Ok(Some(Self {
            finalization: finalization.clone(),
            adapter,
            connection,
            location,
            table,
        }))
    }

    /// Create the schema and table if missing, then append one collection attempt.
    pub(crate) fn append(
        &self,
        run: (&CostRunIdentity<'_>, i64),
        summary: &CostSummary,
    ) -> PyResult<()> {
        let py: Python<'py> = self.adapter.py();
        let keywords = self.location.keywords(py);
        for statement in call_with(
            &native_field(&self.adapter, "render_create_schema")?,
            &keywords,
        )?
        .try_iter()?
        {
            self.execute(&statement?.extract::<String>()?)?;
        }
        let create: String = call_with(
            &native_field(&self.adapter, CREATE_TABLE_METHOD)?,
            &keywords,
        )?
        .extract()?;
        if !create.is_empty() {
            self.execute(&create)?;
        }
        let observed_at: String = call_with(
            &native_field(&self.adapter, "render_current_utc_timestamp")?,
            &[],
        )?
        .extract()?;
        let columns: Vec<&str> = RESOURCE_COST_COLUMN_TYPES
            .iter()
            .map(|(column, _)| *column)
            .collect();
        let cells: Vec<Vec<String>> = observation_cells(py, run, summary, &observed_at)?;
        for chunk in cells.chunks(ROWS_PER_STATEMENT) {
            self.execute(&values_insert_sql(&self.table, &columns, chunk))?;
        }
        Ok(())
    }

    /// The newest run row of each refreshable run; none when the table is missing.
    pub(crate) fn latest(&self) -> PyResult<Vec<RunCostAttempt>> {
        let py: Python<'py> = self.adapter.py();
        let mut keywords = self.location.keywords(py);
        keywords.push(("connection", self.connection.clone()));
        keywords.push((
            "name",
            PyString::new(py, RESOURCE_COSTS_TABLE_NAME).into_any(),
        ));
        if !call_with(&native_field(&self.adapter, "get_columns")?, &keywords)?.is_truthy()? {
            return Ok(Vec::new());
        }
        let result: Bound<'py, PyAny> = self.execute(&latest_run_costs_sql(&self.table))?;
        call_with(&native_field(&result, "fetchall")?, &[])?
            .try_iter()?
            .map(|row| run_attempt(&row?))
            .collect()
    }

    /// Re-collect one older run through the adapter, priced at `rate`.
    pub(crate) fn recollect(
        &self,
        run: &RunCostAttempt,
        (usd_per_credit, rate_source): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.adapter.py();
        let parse = imported(py, "datetime", "datetime")?.getattr("fromisoformat")?;
        let ledger: Bound<'py, PyAny> = native_field(&self.finalization, "project_dir")?
            .call_method1("__truediv__", ("target",))?
            .call_method1("__truediv__", ("executions",))?
            .call_method1("__truediv__", (run.run_id.as_str(),))?
            .call_method1("__truediv__", ("statements.jsonl",))?;
        call_with(
            &native_field(&self.adapter, "collect_run_cost")?,
            &[
                (
                    "connection_config",
                    native_field(&self.finalization, "connection_config")?,
                ),
                (
                    "target_database",
                    native_field(&self.finalization, "target_database")?,
                ),
                ("run_id", PyString::new(py, &run.run_id).into_any()),
                ("started_at", parse.call1((run.run_started_at.as_str(),))?),
                (
                    "completed_at",
                    parse.call1((run.run_completed_at.as_str(),))?,
                ),
                ("statement_ledger_path", ledger),
                ("usd_per_credit", usd_per_credit.clone()),
                ("rate_source", rate_source.clone()),
                (
                    "budget_seconds",
                    PyFloat::new(py, REFRESH_BUDGET_SECONDS).into_any(),
                ),
            ],
        )
    }

    pub(crate) fn close(self) -> PyResult<()> {
        native_field(&self.adapter, "close")?
            .call1((&self.connection,))
            .map(|_| ())
    }

    fn execute(&self, sql: &str) -> PyResult<Bound<'py, PyAny>> {
        execute(&self.adapter, &self.connection, sql)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_resource_costs_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_resource_costs_prune_sql, module)?)
}
