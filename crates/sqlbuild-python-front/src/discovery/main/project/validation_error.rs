//! The Python exception a failed project validation raises.

use crate::discovery::_helpers::failures::raised::{failure_error, read_error};
use crate::discovery::_helpers::project::project_validation::BUILTINS_MODULE;
use crate::discovery::_helpers::project::project_validation::CSV_DIALECT_ERRORS;
use crate::discovery::_helpers::project::project_validation::CSV_MODULE;
use crate::discovery::_helpers::project::project_validation::PATHLIB_MODULE;
use crate::discovery::_helpers::project::project_validation::raised;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{PyAnyMethods, PyErr, Python};
use sqlbuild_discovery::project_validation::models::ValidationFailure;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The Python exception a failed project validation raises.
pub fn validation_error(py: Python<'_>, failure: ValidationFailure) -> PyErr {
    match failure {
        ValidationFailure::Discovery(failure) => failure_error(py, failure),
        ValidationFailure::Unreadable {
            absolute_path,
            failure,
        } => match py
            .import(PATHLIB_MODULE)
            .and_then(|module| native_field(&module, "Path"))
            .and_then(|path| path.call1((absolute_path,)))
        {
            Ok(file_path) => read_error(failure, &file_path),
            Err(error) => error,
        },
        ValidationFailure::Csv(message) => raised(
            py.import(CSV_MODULE)
                .and_then(|module| native_field(&module, "Error"))
                .and_then(|class| class.call1((message,))),
        ),
        ValidationFailure::CsvDialect(error) => {
            if !CSV_DIALECT_ERRORS.contains(&error.class.as_str()) {
                return PyKeyError::new_err(error.class.to_string());
            }
            raised(
                py.import(BUILTINS_MODULE)
                    .and_then(|module| native_field(&module, error.class.as_str()))
                    .and_then(|class| class.call1((error.message,))),
            )
        }
    }
}
