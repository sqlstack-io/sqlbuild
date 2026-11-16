//! Each declared function's name and its UDF and table-function analysis names, sorted.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::BTreeSet;

/// Each declared function's name and its UDF and table-function analysis names, sorted.
pub fn function_names(functions: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for function in functions.try_iter()? {
        let name: String = native_field(&function?, "name")?.extract()?;
        names.insert(format!("__sqlbuild_udf_{name}"));
        names.insert(format!("__sqlbuild_table_function_{name}"));
        names.insert(name);
    }
    Ok(names.into_iter().collect())
}
