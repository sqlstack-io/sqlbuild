//! The argument `name`, passed by keyword or at `position`.

use crate::boundary::_helpers::call_arguments;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// The argument `name`, passed by keyword or at `position`.
#[inline]
pub fn call_argument<'py>(
    (args, kwargs): (&Bound<'py, PyTuple>, Option<&Bound<'py, PyDict>>),
    name: &str,
    position: usize,
) -> PyResult<Bound<'py, PyAny>> {
    call_arguments::call_argument((args, kwargs), name, position)
}
