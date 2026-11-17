//! The model's attached `SchemaModelEntry`, or `None`.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's attached `SchemaModelEntry`, or `None`.
pub fn schema_entry<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.schema_entry(py)?.into_bound(py));
    }
    native_field(model, "schema_entry")
}
