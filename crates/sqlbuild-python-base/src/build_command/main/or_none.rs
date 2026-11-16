//! The value, or `default` when it is `None`.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The value, or `default` when it is `None`.
#[inline]
pub fn or_none<'py>(
    value: Bound<'py, PyAny>,
    default: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    support::or_none(value, default)
}
