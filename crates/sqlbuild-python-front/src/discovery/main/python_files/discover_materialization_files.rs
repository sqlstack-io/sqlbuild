//! Custom materialization Python files under materializations/.

use crate::discovery::_helpers::python_files::imported_python_nodes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Custom materialization Python files under materializations/.
#[inline]
pub fn discover_materialization_files<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_python_nodes::discover_materialization_files(project_dir, providers)
}
