//! Boolean.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::Python;

/// Boolean.
#[inline]
pub fn boolean(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    support::boolean(py, value)
}
