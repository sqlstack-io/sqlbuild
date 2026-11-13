//! Record a build's run cost in the target's state schema, then re-read older incomplete runs.

use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::cost_command::main::accepts_cost_refresh::accepts_cost_refresh;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_core::cost_report::models::{CostRun, CostSummary};
use sqlbuild_state::resource_costs::constants::COLLECTION_FAILED_STATUS;
use sqlbuild_state::resource_costs::main::refresh_candidates::refresh_candidates;
use sqlbuild_state::resource_costs::models::RunCostAttempt;

use crate::bindings::_helpers::cost_report::records::summary;
use crate::bindings::_helpers::native_host::resource_costs_state::{
    CostRunIdentity, ResourceCostStore,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// The attempt number of a build's own collection; each accepted re-read appends the next.
const FIRST_ATTEMPT: i64 = 1;

fn coverage(summary: &CostSummary) -> CostCoverage {
    CostCoverage {
        status: summary.status.clone(),
        expected_statement_count: summary.expected_statement_count,
        observed_statement_count: summary.observed_statement_count,
        query_count: summary.query_count,
        limitations: summary.limitations.clone(),
    }
}

fn unix_now() -> PyResult<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

/// Append each accepted, changed re-read of the build's older incomplete runs.
fn refresh_older(
    store: &ResourceCostStore<'_>,
    run_id: &str,
    rate: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let latest: Vec<RunCostAttempt> = store
        .latest()?
        .into_iter()
        .filter(|latest| latest.run_id != run_id)
        .collect();
    for older in refresh_candidates(&latest, unix_now()?) {
        let Ok(refreshed) = store
            .recollect(&older, rate)
            .and_then(|collected| summary(&collected))
        else {
            continue;
        };
        let refreshed_coverage: CostCoverage = coverage(&refreshed);
        let unchanged: bool = refreshed_coverage == older.coverage
            && older.coverage.status != COLLECTION_FAILED_STATUS;
        if unchanged || accepts_cost_refresh(&older.coverage, &refreshed_coverage) != Some(true) {
            continue;
        }
        store.append(
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
}

/// Append the run's collection, then the accepted re-reads of up to a few older incomplete runs.
pub(crate) fn publish_resource_costs(
    finalization: &Bound<'_, PyAny>,
    run: &CostRun,
    rate: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let Some(store) = ResourceCostStore::open(finalization)? else {
        return Ok(());
    };
    let orchestration_run_id: Option<String> =
        native_field(finalization, "orchestration_run_id")?.extract()?;
    let published: PyResult<()> = store
        .append(
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
        )
        .and_then(|()| refresh_older(&store, &run.run_id, rate));
    let closed: PyResult<()> = store.close();
    published?;
    closed
}
