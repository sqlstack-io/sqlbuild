//! Refuse an unknown limit, a memory size Docker cannot read, and CPUs or a process limit that is not positive.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use pyo3::prelude::PyResult;
use serde_json::Value;
use sqlbuild_orchestration::declarations::models::RunResources;

/// Refuse an unknown limit, a memory size Docker cannot read, and CPUs or a process limit that is not positive.
#[inline]
pub fn run_resources(
    name: &str,
    written: &serde_json::Map<String, Value>,
) -> PyResult<RunResources> {
    orchestration_declarations::run_resources(name, written)
}
