//! `callable(**pairs)`.

use crate::boundary::_helpers::python_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `callable(**pairs)`.
#[inline]
pub fn call_with<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    python_calls::call_with(callable, pairs)
}
