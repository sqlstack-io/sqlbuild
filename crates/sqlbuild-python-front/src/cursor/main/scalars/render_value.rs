//! A scalar in the existing cursor output format.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A scalar in the existing cursor output format.
#[inline]
pub fn render_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    scalars::render_value(value)
}
