//! A new `id` record built exactly as calling its class builds it, finishers included.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::RecordId;
use pyo3::call::PyCallArgs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// A new `id` record built exactly as calling its class builds it, finishers included.
#[inline]
pub fn new_record<'py, A: PyCallArgs<'py>>(
    py: Python<'py>,
    id: RecordId,
    args: A,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    construct::new_record(py, id, args, kwargs)
}
