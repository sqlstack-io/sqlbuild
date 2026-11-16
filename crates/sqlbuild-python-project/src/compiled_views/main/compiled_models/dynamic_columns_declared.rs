//! Whether the attached schema declares dynamic column families.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether the attached schema declares dynamic column families.
pub fn dynamic_columns_declared(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.dynamic_columns_declared(model.py());
    }
    native_field(model, "has_dynamic_columns")?.is_truthy()
}
