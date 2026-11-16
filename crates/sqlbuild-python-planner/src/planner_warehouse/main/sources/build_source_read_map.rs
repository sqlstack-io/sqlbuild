//! Resolve source reads for selected SQL, or for `read_source_names` when given.

use crate::planner_warehouse::_helpers::sources::source_deferral;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyDict as KeywordDict};

/// Resolve source reads for selected SQL, or for `read_source_names` when given.
#[inline]
pub fn build_source_read_map<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    source_deferral::build_source_read_map(py, options)
}
