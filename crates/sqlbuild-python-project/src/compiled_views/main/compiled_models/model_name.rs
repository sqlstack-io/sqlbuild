//! The model's name.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's name.
pub fn model_name(model: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Some(view) = native_model(model)? {
        return view.name(model.py());
    }
    native_field(model, "name")?.extract()
}
