//! The compiled function's `entry_point`, as its Python attribute reads it.

use crate::compiled_views::_helpers::function_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled function's `entry_point`, as its Python attribute reads it.
#[inline]
pub fn entry_point<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    function_reads::entry_point(function)
}
