//! `format_compile_json`, first handing the selected models' partial-check notice to `notice`.

use crate::compile_command::_helpers::invocation::report::compile_json;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyString;

/// `format_compile_json`, first handing the selected models' partial-check notice to `notice`.
pub fn format_compile_json_with_notice<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
    notice: impl FnOnce(Bound<'py, PyString>) -> PyResult<()>,
) -> PyResult<String> {
    compile_json(py, options, Some(notice))
}
