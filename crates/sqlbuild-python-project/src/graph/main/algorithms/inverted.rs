//! Reversed edges: every source key first, then each new target in first-seen order.

use crate::graph::_helpers::algorithms;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Reversed edges: every source key first, then each new target in first-seen order.
#[inline]
pub fn inverted<'py>(edges: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    algorithms::inverted(edges)
}
