//! Each declared column of the attached schema as `(name, type)`, `None` without a schema.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use crate::compiled_views::types::DeclaredRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each declared column of the attached schema as `(name, type)`, `None` without a schema.
pub fn declared_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<DeclaredRow>>> {
    if let Some(view) = native_model(model)? {
        return view.schema_column_rows(model.py());
    }
    native_field(model, "schema_column_rows")?.extract()
}
