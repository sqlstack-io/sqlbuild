//! Format seed command execution results as JSON.

use crate::result_output::_helpers::results::assets::seed_assets;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_host::models::{ExecutionStatus, SeedExecutionResult};
use sqlbuild_python_base::native_host::types::ReadResult;

/// Format seed command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results, plan))]
pub fn format_seed_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
    plan: Bound<'py, PyAny>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let all = Vec::<SeedExecutionResult>::read(&results)?;
    let results = projection.retained(&all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.status == ExecutionStatus::Failed)
        .count();
    emitted(
        &projection,
        Document {
            command: "seed".to_owned(),
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
                    ("total_count", results.len()),
                ],
            )?,
            assets: seed_assets(&projection, &results, Some(&plan))?,
            checks: Vec::new(),
        },
        Extras::default(),
    )
}
