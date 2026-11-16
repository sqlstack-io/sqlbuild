//! The declared column facts of `model_input`'s attached schema, empty without one.

use crate::compiled_project::main::python_column_facts::python_column_facts;
use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The declared column facts of `model_input`'s attached schema, empty without one.
pub fn attached_columns(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<SchemaColumnFact>> {
    let py: Python<'_> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read_with_owner(py, |owner, record| {
            Ok(owner
                .input_schema_fact(record)
                .map(|fact| fact.columns.clone())
                .unwrap_or_default())
        });
    }
    let entry: Bound<'_, PyAny> = native_field(model_input, "schema_entry")?;
    if entry.is_none() {
        return Ok(Vec::new());
    }
    python_column_facts(&native_field(&entry, "columns")?, None)
}
