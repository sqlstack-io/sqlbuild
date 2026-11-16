//! An aware freshness `datetime` in UTC; a naive value is an observation error.

use crate::freshness::_helpers::observation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// An aware freshness `datetime` in UTC; a naive value is an observation error.
#[inline]
pub fn require_aware_utc_datetime<'py>(
    value: &Bound<'py, PyAny>,
    field_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    observation::require_aware_utc_datetime(value, field_name)
}
