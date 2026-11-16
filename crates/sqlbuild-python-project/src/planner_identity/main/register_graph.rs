//! Register the graph functions and classes.

use crate::planner_identity::_helpers::graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the graph functions and classes.
#[inline]
pub fn register_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    graph::register(module)
}
