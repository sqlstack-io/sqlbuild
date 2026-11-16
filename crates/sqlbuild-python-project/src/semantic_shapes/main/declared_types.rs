//! Types named by relation shapes and function signatures, sorted, `UNKNOWN` left out.

use crate::semantic_shapes::_helpers::declarations::UNKNOWN;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use std::collections::BTreeSet;

/// Types named by relation shapes and function signatures, sorted, `UNKNOWN` left out.
pub fn declared_types(
    functions: &Bound<'_, PyAny>,
    column_types: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut types: BTreeSet<String> = BTreeSet::new();
    for columns in mapping_values(column_types)?.try_iter()? {
        for column_type in mapping_values(&columns?)?.try_iter()? {
            types.insert(column_type?.extract()?);
        }
    }
    for function in functions.try_iter()? {
        let function: Bound<'_, PyAny> = function?;
        for argument in native_field(&function, "arguments")?.try_iter()? {
            types.insert(native_field(&argument?, "type")?.extract()?);
        }
        let returns: Bound<'_, PyAny> = native_field(&function, "return_columns")?;
        for column in returns.try_iter()? {
            types.insert(native_field(&column?, "type")?.extract()?);
        }
        if !returns.is_truthy()? {
            types.insert(native_field(&function, "returns")?.extract()?);
        }
    }
    types.remove(UNKNOWN);
    Ok(types.into_iter().collect())
}
