//! The error a plan preview raises for a read only a live plan makes.

use crate::planner_warehouse::_helpers::synced::synced_reads::refused;
use pyo3::PyErr;

/// The error a plan preview raises for a read only a live plan makes.
#[inline]
pub fn refused_access(access: &str) -> PyErr {
    refused(access)
}
