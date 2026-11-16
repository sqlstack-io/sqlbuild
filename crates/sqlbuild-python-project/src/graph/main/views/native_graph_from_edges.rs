//! A native graph holding these edges in their order, with no selector indexes.

use crate::graph::_helpers::views;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A native graph holding these edges in their order, with no selector indexes.
#[inline]
pub fn native_graph_from_edges(
    upstream: &Bound<'_, PyAny>,
    downstream: &Bound<'_, PyAny>,
) -> PyResult<NativeProjectGraph> {
    views::native_graph_from_edges(upstream, downstream)
}
