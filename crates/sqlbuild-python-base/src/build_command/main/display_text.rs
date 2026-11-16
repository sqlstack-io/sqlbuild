//! A string, integer or float shown as text; anything else is unset.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A string, integer or float shown as text; anything else is unset.
#[inline]
pub fn display_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    support::display_text(value)
}
