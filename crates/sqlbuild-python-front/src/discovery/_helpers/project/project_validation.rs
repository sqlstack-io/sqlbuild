//! Cross-file validation of one discovery's inputs.

use crate::discovery::main::project::project_facts::project_facts;
use crate::discovery::main::project::validation_error::validation_error;
use crate::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_discovery::project_validation::main::validate_project_inputs::validate_project_inputs;
use sqlbuild_discovery::project_validation::models::ProjectFacts;

use crate::discovery::models::NodeRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const CSV_MODULE: &str = "csv";
pub(crate) const BUILTINS_MODULE: &str = "builtins";
pub(crate) const PATHLIB_MODULE: &str = "pathlib";
/// The exceptions a CSV dialect may raise, by class name.
pub(crate) const CSV_DIALECT_ERRORS: [&str; 2] = ["TypeError", "ValueError"];

pub(crate) fn raised(result: PyResult<Bound<'_, PyAny>>) -> PyErr {
    match result {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// Raise the first cross-file conflict across the discovered inputs, as Python raises it.
#[pyfunction]
pub(crate) fn validate_discovered_project_inputs(
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
