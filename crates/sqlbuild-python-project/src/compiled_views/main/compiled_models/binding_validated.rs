//! Whether the native session validated the model's bindings.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether the native session validated the model's bindings.
pub fn binding_validated(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.binding_validated(model.py());
    }
    native_field(model, "binding_validated")?.is_truthy()
}
