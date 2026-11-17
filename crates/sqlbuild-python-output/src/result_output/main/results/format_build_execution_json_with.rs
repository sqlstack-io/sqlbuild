//! Production build completion already owns the converted executor result.

use crate::result_output::_helpers::results::documents::TYPED_BUILD_OPTIONS;
use crate::result_output::_helpers::results::documents::degradable_build_json;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_host::models::BuildExecutionResult;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Production build completion already owns the converted executor result.
pub fn format_build_execution_json_with<'py>(
    py: Python<'py>,
    result: &BuildExecutionResult<Bound<'py, PyAny>>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments = KeywordArguments::read(
        py,
        "format_build_execution_json",
        options,
        TYPED_BUILD_OPTIONS,
    )?;
    degradable_build_json(py, &arguments, result)
}
