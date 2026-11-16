//! Apply CLI, project, and model batch limits in their documented precedence order.

use crate::planner_strategy::_helpers::microbatch_limits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Apply CLI, project, and model batch limits in their documented precedence order.
#[inline]
pub fn apply_configured_microbatch_limits<'py>(
    entry: &Bound<'py, PyAny>,
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    microbatch_limits::apply_configured_microbatch_limits(entry, inputs)
}
