//! A native graph holding these edges in their order, with no selector indexes.

use crate::graph::_helpers::project::views::graph_from_dicts;
use crate::graph::_helpers::project::views::native_edges;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_analysis::graph::models::GraphIndexes;

/// A native graph holding these edges in their order, with no selector indexes.
#[pyfunction]
#[pyo3(signature = (*, upstream, downstream))]
pub fn native_graph_from_edges(
    upstream: &Bound<'_, PyAny>,
    downstream: &Bound<'_, PyAny>,
) -> PyResult<NativeProjectGraph> {
    graph_from_dicts(GraphIndexes {
        names: Vec::new(),
        upstream: native_edges(upstream)?,
        downstream: native_edges(downstream)?,
        tags: Vec::new(),
        paths: Vec::new(),
    })
}
