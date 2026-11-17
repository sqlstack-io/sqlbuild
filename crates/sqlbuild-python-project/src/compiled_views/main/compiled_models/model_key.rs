//! The model's `CompiledObjectKey`.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's `CompiledObjectKey`.
pub fn model_key<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.key(model.py())?.into_bound(model.py()));
    }
    native_field(model, "key")
}
