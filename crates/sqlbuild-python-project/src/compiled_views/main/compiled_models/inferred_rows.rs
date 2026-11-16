//! Each inferred output column as `(name, type, nullability)`, `None` when none were inferred.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each inferred output column as `(name, type, nullability)`, `None` when none were inferred.
pub fn inferred_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<InferredRow>>> {
    if let Some(view) = native_model(model)? {
        return view.inferred_rows(model.py());
    }
    let rows: Bound<'_, PyAny> = native_field(model, "inferred_rows")?;
    if rows.is_none() {
        return Ok(None);
    }
    let mut inferred: Vec<InferredRow> = Vec::new();
    for row in rows.try_iter()? {
        let (name, column_type, nullability): (String, Option<String>, Bound<'_, PyAny>) =
            row?.extract()?;
        inferred.push((name, column_type, nullability.str()?.extract()?));
    }
    Ok(Some(inferred))
}
