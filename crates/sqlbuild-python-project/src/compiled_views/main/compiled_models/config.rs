//! The model's `CompileModelConfig`.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's `CompileModelConfig`.
pub fn config<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.config(py)?.into_bound(py));
    }
    native_field(model, "config")
}
