//! Text.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::Python;

/// Text.
#[inline]
pub fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    support::text(py, value)
}
