//! `datetime.now(UTC)`.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `datetime.now(UTC)`.
#[inline]
pub fn utc_now(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    support::utc_now(py)
}
