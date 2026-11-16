//! `len(facts)`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `len(facts)`.
pub fn facts_len(
    instance: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    native_field(instance, "rows")?
        .len()?
        .into_py_any(instance.py())
}
