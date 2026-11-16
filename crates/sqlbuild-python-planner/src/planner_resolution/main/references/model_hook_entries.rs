//! One model's pre and post hooks with their SQL resolved.

use crate::planner_resolution::_helpers::references::orchestration;
use crate::planner_resolution::models::Resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// One model's pre and post hooks with their SQL resolved.
#[inline]
pub fn model_hook_entries<'py>(
    model: &Bound<'py, PyAny>,
    resolution: &Resolution<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    orchestration::model_hook_entries(model, resolution)
}
