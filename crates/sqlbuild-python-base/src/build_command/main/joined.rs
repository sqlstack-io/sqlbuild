//! `base / part`.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `base / part`.
#[inline]
pub fn joined<'py>(base: &Bound<'py, PyAny>, parts: &[&str]) -> PyResult<Bound<'py, PyAny>> {
    support::joined(base, parts)
}
