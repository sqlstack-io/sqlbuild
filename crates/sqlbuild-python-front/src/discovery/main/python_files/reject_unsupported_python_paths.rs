//! Raise `ProjectPythonPathError` naming every unsupported Python file below `project_dir`.

use crate::discovery::_helpers::python_files::python_paths;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Raise `ProjectPythonPathError` naming every unsupported Python file below `project_dir`.
#[inline]
pub fn reject_unsupported_python_paths(project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
    python_paths::reject_unsupported_python_paths(project_dir)
}
