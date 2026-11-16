//! The explicit effective batch-size token resolved against the replay grain.

use crate::plan_output::_helpers::display;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// The explicit effective batch-size token resolved against the replay grain.
#[inline]
pub fn resolve_effective_microbatch_batch_size(
    py: Python<'_>,
    batch_size: &str,
    effective_grain: &str,
) -> Result<String, PyErr> {
    display::resolve_effective_microbatch_batch_size(py, batch_size, effective_grain)
}
