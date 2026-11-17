//! Audit reporting from already-converted executor results, before the document is serialized.

use crate::result_output::_helpers::results::checks::audit_checks;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::models::{AuditExecutionResult, AuditOutcome};
use sqlbuild_python_planner::plan_output::main::any::any;
use sqlbuild_python_planner::plan_output::main::dict::dict;

/// Audit reporting from already-converted executor results, before the document is serialized.
pub fn audit_execution_json<'py>(
    py: Python<'py>,
    all: &[AuditExecutionResult<Bound<'py, PyAny>>],
    (configured_concurrency, worker_count): (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>),
    trailing: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<String> {
    let configured_concurrency: Bound<'py, PyAny> =
        configured_concurrency.map_or_else(|| any(py, 1_i64), Ok)?;
    let projection: Projection<'py> = Projection::installed(py)?;
    let results = projection.retained(all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.outcome == AuditOutcome::Error)
        .count();
    let workers: Bound<'py, PyAny> = match worker_count.filter(|count| !count.is_none()) {
        Some(workers) => workers,
        None => py
            .import("builtins")?
            .getattr("min")?
            .call1((&configured_concurrency, results.len()))?,
    };
    emitted(
        &projection,
        Document {
            command: "audit".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    (
                        "pass_count",
                        results
                            .iter()
                            .filter(|result| result.outcome == AuditOutcome::Pass)
                            .count(),
                    ),
                    (
                        "warn_count",
                        results
                            .iter()
                            .filter(|result| result.outcome == AuditOutcome::Warn)
                            .count(),
                    ),
                    ("fail_count", fail_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: audit_checks(&projection, &results, false)?,
        },
        Extras {
            execution: Some(dict(
                py,
                vec![
                    ("configured_concurrency", configured_concurrency),
                    ("worker_count", workers),
                ],
            )?),
            trailing,
            ..Extras::default()
        },
    )
}
