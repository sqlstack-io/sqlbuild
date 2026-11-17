//! Resolve explicit or default lookback for one microbatch model.

use crate::planner_strategy::_helpers::model_entries::lookback_of;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Resolve explicit or default lookback for one microbatch model.
#[pyfunction]
#[pyo3(signature = (*, model, effective_grain = None))]
pub fn resolve_microbatch_lookback(
    model: &Bound<'_, PyAny>,
    effective_grain: Option<String>,
) -> PyResult<Option<String>> {
    lookback_of(model, effective_grain)
}
