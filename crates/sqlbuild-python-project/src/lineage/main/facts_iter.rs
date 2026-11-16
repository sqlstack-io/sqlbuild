//! `iter(facts)`.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `iter(facts)`.
pub fn facts_iter(
    instance: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    Ok(native_field(instance, "facts")?
        .try_iter()?
        .into_any()
        .unbind())
}
