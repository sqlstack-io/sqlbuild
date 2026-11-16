//! Withhold the launch record: `sqb` cannot replay what this invocation also depends on.

use crate::cli::_helpers::launch_state;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Withhold the launch record: `sqb` cannot replay what this invocation also depends on.
#[inline]
pub fn withhold_launch_record(py: Python<'_>) -> PyResult<()> {
    launch_state::withhold_launch_record(py)
}
