//! Reuse collected columns and inspect only selected, unresolved table sources.

use crate::planner_warehouse::_helpers::sources::semantic_sources;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyDict as KeywordDict};

/// Reuse collected columns and inspect only selected, unresolved table sources.
#[inline]
pub fn semantic_source_columns<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    semantic_sources::semantic_source_columns(py, options)
}
