//! A snapshot model's validity column names, which the snapshot adds to its query's columns.

use crate::semantic_shapes::constants::VALID_FROM_DEFAULT;
use crate::semantic_shapes::constants::VALID_TO_DEFAULT;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// A snapshot model's validity column names, which the snapshot adds to its query's columns.
pub fn snapshot_validity_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let py: Python<'_> = model.py();
    let values: Bound<'_, PyAny> = native_field(&native_field(model, "config")?, "values")?;
    let snapshot: Bound<'_, PyAny> = enum_member(py, RecordId::MaterializationType, "SNAPSHOT")?;
    if !mapping_get(&values, "materialized")?.eq(snapshot)? {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = Vec::with_capacity(2);
    for (key, default) in [
        ("valid_from_column", VALID_FROM_DEFAULT),
        ("valid_to_column", VALID_TO_DEFAULT),
    ] {
        let name: Bound<'_, PyAny> = mapping_get(&values, key)?;
        names.push(if name.is_truthy()? {
            name.str()?.extract()?
        } else {
            default.to_owned()
        });
    }
    Ok(names)
}
