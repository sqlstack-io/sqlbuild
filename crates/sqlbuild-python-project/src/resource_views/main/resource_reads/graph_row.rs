//! A resource's graph facts without constructing its public keys or dependency tuple.

use crate::graph::models::ResourceRow;
use crate::resource_views::_helpers::resource_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A resource's graph facts without constructing its public keys or dependency tuple.
#[inline]
pub fn graph_row(resource: &Bound<'_, PyAny>) -> PyResult<ResourceRow> {
    resource_reads::graph_row(resource)
}
