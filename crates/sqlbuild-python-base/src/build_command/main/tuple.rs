//! Tuple.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Tuple.
#[inline]
pub fn tuple<'py>(py: Python<'py>, values: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    support::tuple(py, values)
}
