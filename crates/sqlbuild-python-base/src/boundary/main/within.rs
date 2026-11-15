//! Run `body` inside `with manager as entered:`; managers here never suppress exceptions.

use crate::boundary::_helpers::python_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Run `body` inside `with manager as entered:`; managers here never suppress exceptions.
#[inline]
pub fn within<'py, T>(
    manager: &Bound<'py, PyAny>,
    body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
) -> PyResult<T> {
    python_calls::within(manager, body)
}
