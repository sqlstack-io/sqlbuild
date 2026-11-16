//! Decorated Python DAG node functions under python/.

use crate::discovery::_helpers::python_files::imported_python_nodes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Decorated Python DAG node functions under python/.
#[inline]
pub fn discover_python_node_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_python_nodes::discover_python_node_functions(project_dir, providers)
}
