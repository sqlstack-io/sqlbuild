//! The model's compiled query SQL.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's compiled query SQL.
pub fn query_sql<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.query_sql(model.py())?.into_any());
    }
    native_field(model, "query_sql")
}
