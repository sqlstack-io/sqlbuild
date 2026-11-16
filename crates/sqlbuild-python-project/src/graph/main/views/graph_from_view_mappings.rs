//! A native graph over Python's index mappings, keeping their order.

use crate::graph::_helpers::views;
use crate::graph::models::GraphViewMappings;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::PyResult;

/// A native graph over Python's index mappings, keeping their order.
#[inline]
pub fn graph_from_view_mappings(
    mappings: GraphViewMappings<'_, '_>,
) -> PyResult<NativeProjectGraph> {
    views::graph_from_view_mappings(mappings)
}
