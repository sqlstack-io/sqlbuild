//! Authored MODEL and SCHEMA columns parsed natively into contract columns.

use crate::model_config::main::config::native_config_error::native_config_error;
use crate::model_config::main::headers::schema_columns::schema_columns;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_model_config::errors::{ConfigError, ErrorClass};

use crate::model_config::_helpers::config::config_errors::config_exception;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// `(file_path, label)`: the path and label every message names.
pub(crate) type ColumnSite<'a> = (&'a str, &'a str);
/// `(require_columns, allow_migrate_from, declaration_errors)`.
pub(crate) type ColumnOptions = (bool, bool, bool);

/// Parse one MODEL or SCHEMA `columns` mapping into contract columns.
#[pyfunction]
pub(crate) fn parse_schema_columns<'py>(
    raw_columns: Bound<'py, PyAny>,
    site: (String, String),
    options: ColumnOptions,
    column_locations: Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyTuple>> {
    compiler_guard(|| schema_columns(&raw_columns, (&site.0, &site.1), options, &column_locations))
}

/// A plain parse error as `DeclarationParseError` when asked; every other error as Python raises it.
pub(crate) fn raised(py: Python<'_>, error: ConfigError, declaration_errors: bool) -> PyErr {
    let built = (|| {
        if declaration_errors && error.class == ErrorClass::CompileInput {
            return error_class(py, ErrorId::DeclarationParse)?.call1((error.message,));
        }
        let native = native_config_error(py, error)?.into_bound(py).into_any();
        config_exception(&native, None)
    })();
    match built {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}
