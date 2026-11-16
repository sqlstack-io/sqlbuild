//! The function names and declared types native binding treats as known.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;

const UNKNOWN: &str = "UNKNOWN";

/// Each declared function's name and its UDF and table-function analysis names, sorted.
pub(crate) fn function_names(functions: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for function in functions.try_iter()? {
        let name: String = native_field(&function?, "name")?.extract()?;
        names.insert(format!("__sqlbuild_udf_{name}"));
        names.insert(format!("__sqlbuild_table_function_{name}"));
        names.insert(name);
    }
    Ok(names.into_iter().collect())
}

/// Types named by relation shapes and function signatures, sorted, `UNKNOWN` left out.
pub(crate) fn declared_types(
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
