//! Resolve explicit or default lookback for one microbatch model.

use crate::planner_strategy::_helpers::model_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve explicit or default lookback for one microbatch model.
#[inline]
pub fn resolve_microbatch_lookback(
    model: &Bound<'_, PyAny>,
    effective_grain: Option<String>,
) -> PyResult<Option<String>> {
    model_entries::resolve_microbatch_lookback(model, effective_grain)
}
