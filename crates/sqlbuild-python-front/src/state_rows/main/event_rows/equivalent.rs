//! Whether two source freshness records are equivalent for skip decisions.

use crate::state_rows::_helpers::event_rows::freshness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether two source freshness records are equivalent for skip decisions.
#[inline]
pub fn equivalent(
    previous_record: &Bound<'_, PyAny>,
    current_record: &Bound<'_, PyAny>,
    lag_tolerance: Option<&str>,
) -> PyResult<bool> {
    freshness::equivalent(previous_record, current_record, lag_tolerance)
}
