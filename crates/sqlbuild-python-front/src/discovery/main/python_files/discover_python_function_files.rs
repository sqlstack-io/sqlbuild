//! Discover the `@udf(...)` Python function files under `functions/python/`.

use crate::discovery::_helpers::python_files::python_function_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover the `@udf(...)` Python function files under `functions/python/`.
#[inline]
pub fn discover_python_function_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    python_function_files::discover_python_function_files(project_dir)
}
