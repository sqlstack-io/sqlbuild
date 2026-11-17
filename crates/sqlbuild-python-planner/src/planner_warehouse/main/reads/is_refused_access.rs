//! Whether an error is a plan preview's refusal of a warehouse read.

use crate::planner_warehouse::_helpers::synced::exception_types::WarehouseAccessRefusedError;
use pyo3::{PyErr, Python};

/// Whether `error` is a plan preview's refusal of a warehouse read.
#[inline]
pub fn is_refused_access(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<WarehouseAccessRefusedError>(py)
}
