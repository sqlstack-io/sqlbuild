//! A source freshness value normalized for stable state storage.

use crate::state_rows::_helpers::event_rows::freshness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A source freshness value normalized for stable state storage.
#[inline]
pub fn normalize<'py>(
    value: &Bound<'py, PyAny>,
    value_kind: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    freshness::normalize(value, value_kind)
}
