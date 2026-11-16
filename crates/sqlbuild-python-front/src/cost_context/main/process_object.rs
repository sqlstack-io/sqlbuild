//! The process-wide Python object in a cell, built on first use.

use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;

/// The process-wide Python object in `cell`, built by `build` on first use.
pub fn process_object<'py>(
    cell: &'static PyOnceLock<Py<PyAny>>,
    py: Python<'py>,
    build: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<&'py Bound<'py, PyAny>> {
    cell.get_or_try_init(py, || Ok::<Py<PyAny>, PyErr>(build()?.unbind()))
        .map(|object| object.bind(py))
}
