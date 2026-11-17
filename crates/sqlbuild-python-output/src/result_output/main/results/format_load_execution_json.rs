//! Format load command execution results as JSON.

use crate::result_output::_helpers::results::assets::load_assets;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_host::models::{ExecutionStatus, LoadExecutionResult};
use sqlbuild_python_base::native_host::types::ReadResult;

/// Format load command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results))]
pub fn format_load_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let all = Vec::<LoadExecutionResult<_>>::read(&results)?;
    let results = projection.retained(&all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.status == ExecutionStatus::Failed)
        .count();
    emitted(
        &projection,
        Document {
            command: "load".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    (
                        "success_count",
                        results
                            .iter()
                            .filter(|result| result.status == ExecutionStatus::Success)
                            .count(),
                    ),
                    ("failure_count", fail_count),
                    (
                        "skipped_count",
                        results
                            .iter()
                            .filter(|result| result.status == ExecutionStatus::Skipped)
                            .count(),
                    ),
                    ("total_count", results.len()),
                ],
            )?,
            assets: load_assets(&projection, &results)?,
            checks: Vec::new(),
        },
        Extras::default(),
    )
}
