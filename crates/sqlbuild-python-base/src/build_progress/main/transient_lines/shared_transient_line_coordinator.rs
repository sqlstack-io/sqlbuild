//! Return the coordinator shared by every terminal writer in this process.

use crate::build_progress::_helpers::transient_lines;
use crate::build_progress::models::TransientLineCoordinator;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Return the coordinator shared by every terminal writer in this process.
#[inline]
pub fn shared_transient_line_coordinator(
    py: Python<'_>,
) -> PyResult<Bound<'_, TransientLineCoordinator>> {
    transient_lines::shared_transient_line_coordinator(py)
}
