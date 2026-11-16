//! The Python node names `select` minus `exclude` resolve to; every node when unselected.

use crate::graph::_helpers::python_nodes::python_selection;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// The Python node names `select` minus `exclude` resolve to; every node when unselected.
#[inline]
pub fn python_node_names<'py>(
    graph: &Bound<'py, PyAny>,
    (select, exclude): (Vec<String>, Vec<String>),
) -> PyResult<Bound<'py, PyFrozenSet>> {
    python_selection::python_node_names(graph, (select, exclude))
}
