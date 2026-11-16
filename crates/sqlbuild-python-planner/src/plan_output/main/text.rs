//! `str(value)`.

use crate::plan_output::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `str(value)`.
#[inline]
pub fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    values::text(value)
}
