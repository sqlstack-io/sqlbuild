//! Format scenario snapshot sync/refresh execution results as JSON.

use crate::result_output::_helpers::results::checks::snapshot_result;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::FAILED;
use crate::result_output::_helpers::results::documents::SNAPSHOT_OPTIONS;
use crate::result_output::_helpers::results::documents::count_status;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::namespace_execution;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::passed;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_planner::plan_output::main::any::any;
use sqlbuild_python_planner::plan_output::main::items::items;

/// Format scenario snapshot sync/refresh execution results as JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn format_scenario_snapshot_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "format_scenario_snapshot_execution_json",
        options,
        SNAPSHOT_OPTIONS,
    )?;
    let results: Bound<'py, PyAny> = arguments.value("results")?;
    let refresh: bool = arguments.passed("refresh")? && arguments.flag("refresh")?;
    let run_namespace: Option<Bound<'py, PyAny>> = passed(&arguments, "run_namespace")?;
    let namespace_source: Option<Bound<'py, PyAny>> = passed(&arguments, "namespace_source")?;
    let projection: Projection<'py> = Projection::installed(py)?;
    let results: Vec<Bound<'py, PyAny>> = items(&results)?;
    let fail_count: usize = count_status(&results, "status", FAILED)?;
    emitted(
        &projection,
        Document {
            command: if refresh {
                "scenario snapshot refresh"
            } else {
                "scenario snapshot sync"
            }
            .to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    ("pass_count", results.len() - fail_count),
                    ("fail_count", fail_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: Vec::new(),
        },
        Extras {
            scenarios: results
                .iter()
                .map(snapshot_result)
                .collect::<PyResult<_>>()?,
            execution: Some(namespace_execution(
                py,
                run_namespace,
                namespace_source.map_or_else(|| any(py, "unset"), Ok)?,
            )?),
            ..Extras::default()
        },
    )
}
