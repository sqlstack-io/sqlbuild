//! Register the project graph functions and classes.

use crate::graph::_helpers::project::project_graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the project graph functions and classes.
#[inline]
pub fn register_project_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project_graph::register(module)
}
