//! Production scenario completion already owns its converted executor results.

use crate::result_output::_helpers::results::documents::SCENARIO_OPTIONS;
use crate::result_output::_helpers::results::documents::scenario_json;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_host::models::ScenarioRunResult;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Production scenario completion already owns its converted executor results.
pub fn format_scenario_execution_json_with<'py>(
    py: Python<'py>,
    results: &[ScenarioRunResult<Bound<'py, PyAny>>],
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments = KeywordArguments::read(
        py,
        "format_scenario_execution_json",
        options,
        (&[], SCENARIO_OPTIONS.1),
    )?;
    scenario_json(py, &arguments, results)
}
