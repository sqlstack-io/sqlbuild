//! Record each collected run's cost in the target's state schema and re-read incomplete runs.
//!
//! This runs only straight after a build's own cost collection, while the warehouse is awake from
//! the build, so it never resumes an idle warehouse.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::cost_command::main::accepts_cost_refresh::accepts_cost_refresh;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_core::cost_report::models::{CostRun, CostSummary};
use sqlbuild_state::resource_costs::constants::COLLECTION_FAILED_STATUS;
use sqlbuild_state::resource_costs::main::refresh_candidates::refresh_candidates;
use sqlbuild_state::resource_costs::models::RunCostAttempt;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::cost_report::records::summary;
use crate::bindings::_helpers::native_host::consumption_state::Location;
use crate::bindings::_helpers::native_host::resource_costs_state::{
    CostRunIdentity, append_resource_costs, latest_run_costs,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// The attempt number of a build's own collection; each accepted re-read appends the next.
const FIRST_ATTEMPT: i64 = 1;
/// Collection budget of each re-read of an older run, which a build waits for.
const REFRESH_BUDGET_SECONDS: f64 = 20.0;
/// The adapter method that marks the resource costs table as supported.
const CREATE_TABLE_METHOD: &str = "render_create_resource_costs_table_sql";

fn coverage(summary: &CostSummary) -> CostCoverage {
    CostCoverage {
        status: summary.status.clone(),
        expected_statement_count: summary.expected_statement_count,
        observed_statement_count: summary.observed_statement_count,
        query_count: summary.query_count,
        limitations: summary.limitations.clone(),
    }
}

/// The target's state location, when the adapter keeps resource costs and the target has a schema.
fn location(finalization: &Bound<'_, PyAny>) -> PyResult<Option<Location>> {
    let adapter: Bound<'_, PyAny> = native_field(finalization, "adapter")?;
    if !adapter.hasattr(CREATE_TABLE_METHOD)? {
        return Ok(None);
    }
    let Some(schema) = native_field(finalization, "target_schema")?.extract::<Option<String>>()?
    else {
        return Ok(None);
    };
    Ok(Some(Location {
        database: native_field(finalization, "target_database")?.extract()?,
        schema,
    }))
}

/// Re-collect one older run with the project's current rate.
fn recollect<'py>(
    finalization: &Bound<'py, PyAny>,
    run: &RunCostAttempt,
    (usd_per_credit, rate_source): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<CostSummary> {
    let py: Python<'py> = finalization.py();
    let parse = imported(py, "datetime", "datetime")?.getattr("fromisoformat")?;
    let ledger: Bound<'py, PyAny> = native_field(finalization, "project_dir")?
        .call_method1("__truediv__", ("target",))?
        .call_method1("__truediv__", ("executions",))?
        .call_method1("__truediv__", (run.run_id.as_str(),))?
        .call_method1("__truediv__", ("statements.jsonl",))?;
    let collected: Bound<'py, PyAny> = call_with(
        &native_field(&native_field(finalization, "adapter")?, "collect_run_cost")?,
        &[
            (
                "connection_config",
                native_field(finalization, "connection_config")?,
            ),
            (
                "target_database",
                native_field(finalization, "target_database")?,
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
                REFRESH_BUDGET_SECONDS.into_pyobject(py)?.into_any(),
            ),
        ],
    )?;
    summary(&collected)
}

/// Append the run's collection, then the accepted re-reads of up to a few older incomplete runs.
pub(crate) fn publish_resource_costs(
    finalization: &Bound<'_, PyAny>,
    run: &CostRun,
    rate: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let Some(location) = location(finalization)? else {
        return Ok(());
    };
    let py: Python<'_> = finalization.py();
    let adapter: Bound<'_, PyAny> = native_field(finalization, "adapter")?;
    let orchestration_run_id: Option<String> =
        native_field(finalization, "orchestration_run_id")?.extract()?;
    let connection: Bound<'_, PyAny> = adapter.call_method1(
        "connect",
        (native_field(finalization, "connection_config")?,),
    )?;
    let published: PyResult<()> = (|| {
        append_resource_costs(
            &adapter,
            &connection,
            &location,
            (
                &CostRunIdentity {
                    run_id: &run.run_id,
                    orchestration_run_id: orchestration_run_id.as_deref(),
                    target_name: run.target_name.as_deref(),
                    started_at: &run.started_at,
                    completed_at: &run.completed_at,
                },
                FIRST_ATTEMPT,
            ),
            &run.cost,
        )?;
        let now: i64 = imported(py, "time", "time")?.call0()?.extract::<f64>()? as i64;
        let latest: Vec<RunCostAttempt> = latest_run_costs(&adapter, &connection, &location)?
            .into_iter()
            .filter(|latest| latest.run_id != run.run_id)
            .collect();
        for older in refresh_candidates(&latest, now) {
            let Ok(refreshed) = recollect(finalization, &older, rate) else {
                continue;
            };
            let refreshed_coverage: CostCoverage = coverage(&refreshed);
            let unchanged: bool = refreshed_coverage == older.coverage
                && older.coverage.status != COLLECTION_FAILED_STATUS;
            if unchanged || accepts_cost_refresh(&older.coverage, &refreshed_coverage) != Some(true)
            {
                continue;
            }
            append_resource_costs(
                &adapter,
                &connection,
                &location,
                (
                    &CostRunIdentity {
                        run_id: &older.run_id,
                        orchestration_run_id: older.orchestration_run_id.as_deref(),
                        target_name: older.target_name.as_deref(),
                        started_at: &older.run_started_at,
                        completed_at: &older.run_completed_at,
                    },
                    older.attempt + 1,
                ),
                &refreshed,
            )?;
        }
        Ok(())
    })();
    let closed: PyResult<Bound<'_, PyAny>> = adapter.call_method1("close", (&connection,));
    published?;
    closed.map(|_| ())
}
