//! Partial-check reasons by model name; empty when SQL analysis is off.

use crate::semantic_shapes::_helpers::coverage;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Partial-check reasons by model name; empty when SQL analysis is off.
#[inline]
pub fn semantic_coverage<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    coverage::semantic_coverage(project)
}
