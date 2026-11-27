//! Python strings for each item of an iterable of `str`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;

/// Python strings for each item of an iterable of `str`.
pub fn py_strings<'py>(values: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyString>>> {
    values
        .try_iter()?
        .map(|value| Ok(value?.cast_into::<PyString>()?))
        .collect()
}
