//! Edge dict.

use crate::graph::_helpers::project::views;
use crate::graph::models::KeyCache;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Edge dict.
#[inline]
pub fn edge_dict<'py>(
    cache: &KeyCache<'py>,
    edges: &[(ObjectKey, Vec<ObjectKey>)],
) -> PyResult<Bound<'py, PyDict>> {
    views::edge_dict(cache, edges)
}
