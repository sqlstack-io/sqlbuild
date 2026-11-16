//! The compiled source's `key`, as its Python attribute reads it.

use crate::resource_views::_helpers::source_reads::native_source;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled source's `key`, as its Python attribute reads it.
pub fn key<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(view) = native_source(source)? {
        return view.key(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(source, "key")
}
