//! The model's name as a Python string.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's name as a Python string.
pub fn name_object<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        let name: String = view.name(model.py())?;
        return Ok(pyo3::types::PyString::new(model.py(), &name).into_any());
    }
    native_field(model, "name")
}
