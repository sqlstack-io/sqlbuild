//! Check full rebuild allowed.

use crate::planner_phases::_helpers::rebuilds::full_refresh;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Check full rebuild allowed.
#[inline]
pub fn check_full_rebuild_allowed(
    model: &Bound<'_, PyAny>,
    change_result: &Bound<'_, PyAny>,
    full_rebuild_cause: &str,
) -> PyResult<()> {
    full_refresh::check_full_rebuild_allowed(model, change_result, full_rebuild_cause)
}
