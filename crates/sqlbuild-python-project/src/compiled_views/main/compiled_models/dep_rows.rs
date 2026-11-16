//! `(resource type, name)` of each dependency.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `(resource type, name)` of each dependency.
pub fn dep_rows(model: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    if let Some(view) = native_model(model)? {
        return view.dep_rows(model.py());
    }
    native_field(model, "dep_rows")?.extract()
}
