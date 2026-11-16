//! The project graph indexes of these resources, in project order.

use crate::graph::_helpers::project::project_graph;
use crate::graph::models::NativeProjectGraph;
use crate::graph::models::ResourceRow;
use pyo3::prelude::PyResult;

/// The project graph indexes of these resources, in project order.
#[inline]
pub fn project_graph(resources: Vec<ResourceRow>) -> PyResult<NativeProjectGraph> {
    project_graph::project_graph(resources)
}
