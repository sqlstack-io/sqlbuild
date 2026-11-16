//! None.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::Python;

/// None.
#[inline]
pub fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    support::none(py)
}
