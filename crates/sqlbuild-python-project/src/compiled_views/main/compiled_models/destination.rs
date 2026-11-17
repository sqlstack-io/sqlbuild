//! The model's destination `CompiledRelationLocation`.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's destination `CompiledRelationLocation`.
pub fn destination<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.destination(model.py())?.into_bound(model.py()));
    }
    native_field(model, "destination")
}
