//! The compiled function's `arguments`, as its Python attribute reads it.

use crate::resource_views::_helpers::function_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled function's `arguments`, as its Python attribute reads it.
#[inline]
pub fn arguments<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    function_reads::arguments(function)
}
