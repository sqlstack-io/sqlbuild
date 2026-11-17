//! The test command's document from the executor's already-converted results.

use crate::result_output::_helpers::results::checks::sql_test_checks;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_host::models::{SqlTestExecutionResult, SqlTestOutcome};

/// The test command's document from the executor's already-converted results.
pub fn test_execution_json<'py>(
    py: Python<'py>,
    all: &[SqlTestExecutionResult<Bound<'py, PyAny>>],
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let results = projection.retained(all)?;
    let pass_count = results
        .iter()
        .filter(|result| result.outcome == SqlTestOutcome::Pass)
        .count();
    let timeout_count = results
        .iter()
        .filter(|result| result.outcome == SqlTestOutcome::Timeout)
        .count();
    let fail_count: usize = results.len() - pass_count;
    emitted(
        &projection,
        Document {
            command: "test".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    ("pass_count", pass_count),
                    ("fail_count", fail_count),
                    ("timeout_count", timeout_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: sql_test_checks(&projection, &results)?,
        },
        Extras::default(),
    )
}
