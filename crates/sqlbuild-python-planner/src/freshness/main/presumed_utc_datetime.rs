//! A `datetime` in UTC, a naive or offset-less value read as UTC.

use crate::freshness::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A `datetime` in UTC, a naive or offset-less value read as UTC.
#[inline]
pub fn presumed_utc_datetime<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    support::presumed_utc_datetime(value)
}
