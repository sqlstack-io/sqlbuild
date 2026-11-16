//! Reject capped producers feeding a microbatch model's watermark inputs.

use crate::planner_strategy::_helpers::lineage;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// Reject capped producers feeding a microbatch model's watermark inputs.
#[inline]
pub fn reject_capped_watermark_inputs<'py>(
    model: &Bound<'py, PyAny>,
    models_by_name: ModelLookup,
    functions_by_name: Option<Bound<'py, PyAny>>,
) -> PyResult<()> {
    lineage::reject_capped_watermark_inputs(model, models_by_name, functions_by_name)
}
