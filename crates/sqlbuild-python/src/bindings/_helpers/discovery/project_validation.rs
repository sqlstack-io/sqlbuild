//! Cross-file validation of one discovery's inputs.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_discovery::project_validation::main::validate_project_inputs::validate_project_inputs;
use sqlbuild_discovery::project_validation::models::{ProjectFacts, ValidationFailure};

use crate::bindings::_helpers::discovery::orchestration_files::orchestration_declarations;
use crate::bindings::_helpers::discovery::project_facts::{NodeRow, project_facts};
use crate::bindings::_helpers::discovery::raised::{failure_error, read_error};
use crate::bindings::_helpers::records::fields::field as native_field;

const CSV_MODULE: &str = "csv";
const BUILTINS_MODULE: &str = "builtins";
const PATHLIB_MODULE: &str = "pathlib";
/// The exceptions a CSV dialect may raise, by class name.
const CSV_DIALECT_ERRORS: [&str; 2] = ["TypeError", "ValueError"];

fn raised(result: PyResult<Bound<'_, PyAny>>) -> PyErr {
    match result {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// Raise the first cross-file conflict across the discovered inputs, as Python raises it.
#[pyfunction]
fn validate_discovered_project_inputs(
    py: Python<'_>,
    inputs: Bound<'_, PyAny>,
    nodes: Vec<NodeRow>,
) -> Result<(), PyErr> {
    let mut facts: ProjectFacts = project_facts(py, &inputs, nodes)?;
    let project_dir: Bound<'_, PyAny> = native_field(&inputs, "project_dir")?;
    if !project_dir.is_none() {
        facts.orchestration = orchestration_declarations(&project_dir)?.names();
    }
    validate_project_inputs(&facts).map_err(|failure| validation_error(py, failure))
}

/// The Python exception a failed project validation raises.
pub(crate) fn validation_error(py: Python<'_>, failure: ValidationFailure) -> PyErr {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        validate_discovered_project_inputs,
        module
    )?)
}
