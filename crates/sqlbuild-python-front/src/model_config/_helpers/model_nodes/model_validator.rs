//! The Python model validators, run natively up to the first error they raise.

use crate::model_config::main::config::native_config_error::native_config_error;
use sqlbuild_python_base::positional_record;
use std::collections::HashSet;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_model_config::model_validation::models::ValidationStop;

use crate::discovery::main::project::column_name::column_name;
use crate::discovery::main::project::each::each;
use crate::discovery::main::project::stem::stem;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

const FILE_PATH_ATTRIBUTE: &str = "file_path";
const RETURNS_KEY: &str = "returns";
/// The only key of a table function's `returns` mapping.
const TABLE_RETURNS_KEY: &str = "table";

positional_record! {
    extract;
    /// Model, seed, source, function and table function names.
    pub(crate) struct ResourceNames {
        pub(crate) models: HashSet<String>,
        pub(crate) seeds: HashSet<String>,
        pub(crate) sources: HashSet<String>,
        pub(crate) functions: HashSet<String>,
        pub(crate) table_functions: HashSet<String>,
    }
}

positional_record! {
    extract;
    /// References `(kind, name, rejected)`, schema columns, unmanaged retention and table type.
    pub(crate) struct ModelFacts {
        pub(crate) references: Vec<(String, String, bool)>,
        pub(crate) declared_columns: Option<Vec<String>>,
        pub(crate) retention_unmanaged: bool,
        pub(crate) table_type_declared: bool,
    }
}

/// Model, seed, source, function and table function names discovered in `inputs`.
pub(crate) fn discovered_names(inputs: &Bound<'_, PyAny>) -> PyResult<ResourceNames> {
    let model_stem = |file: &Bound<'_, PyAny>| stem(file, FILE_PATH_ATTRIBUTE);
    let mut models: HashSet<String> = each(inputs, "model_files", model_stem)?
        .into_iter()
        .collect();
    if let Some(injection) =
        sqlbuild_python_base::compile_context::main::delta::injection::injection()
    {
        models.extend(injection.model_names.iter().cloned());
    }
    let mut seeds: HashSet<String> = HashSet::new();
    for schema in native_field(inputs, "schema_files")?.try_iter()? {
        seeds.extend(each(&schema?, "seed_entries", column_name)?);
    }
    let mut sources: HashSet<String> = HashSet::new();
    for file in native_field(inputs, "source_files")?.try_iter()? {
        sources.extend(each(&file?, "source_entries", column_name)?);
    }
    let mut functions: HashSet<String> = HashSet::new();
    let mut table_functions: HashSet<String> = HashSet::new();
    for file in native_field(inputs, "sql_function_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        let name: String = model_stem(&file)?;
        if returns_table(&file)? {
            table_functions.insert(name.clone());
        }
        functions.insert(name);
    }
    functions.extend(each(inputs, "python_function_files", model_stem)?);
    Ok(ResourceNames {
        models,
        seeds,
        sources,
        functions,
        table_functions,
    })
}

/// Whether a SQL function's header `returns` is a mapping whose only key is `table`.
fn returns_table(file: &Bound<'_, PyAny>) -> PyResult<bool> {
    let headers: Bound<'_, PyAny> = native_field(file, "header_values")?;
    let returns: Bound<'_, PyAny> = mapping_get(&headers, RETURNS_KEY)?;
    let Ok(returns) = returns.downcast::<PyDict>() else {
        return Ok(false);
    };
    let keys: Vec<Bound<'_, PyAny>> = returns.keys().into_iter().collect();
    Ok(keys.len() == 1 && keys[0].eq(TABLE_RETURNS_KEY)?)
}

pub(crate) fn stop_object(py: Python<'_>, stop: ValidationStop) -> PyResult<Py<PyAny>> {
    match stop.into_error() {
        Err(index) => Ok(index.into_pyobject(py)?.into_any().unbind()),
        Ok(error) => Ok(native_config_error(py, error)?.into_any()),
    }
}
