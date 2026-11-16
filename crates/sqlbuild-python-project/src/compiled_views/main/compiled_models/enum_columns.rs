//! The model's enum columns by name.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's enum columns by name.
pub fn enum_columns<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.enum_columns(py)?.into_bound(py).into_any());
    }
    native_field(model, "enum_columns")
}
