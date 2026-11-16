//! The Python node names `sqb check` exclude tokens name, skipping tokens that match nothing.

use crate::graph::_helpers::python_nodes::python_selection;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// The Python node names `sqb check` exclude tokens name, skipping tokens that match nothing.
#[inline]
pub fn resolve_python_check_excludes<'py>(
    graph: &Bound<'py, PyAny>,
    exclude: Vec<String>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    python_selection::resolve_python_check_excludes(graph, exclude)
}
