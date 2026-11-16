//! Return the resource facts and the resource Python's assembly raises at, if any.

use crate::project_assembly::_helpers::native_resources::PROJECT_ASSEMBLY_CONTEXT;
use crate::project_assembly::_helpers::native_resources::project_request;
use crate::project_assembly::_helpers::native_resources::resources_object;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::assembly::project::main::assemble_project_resources::assemble_project_resources;
use sqlbuild_analysis::assembly::project::models::ProjectResources;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// Return the resource facts and the resource Python's assembly raises at, if any.
#[pyfunction]
#[pyo3(signature = (*, inputs, dialect, syntax_checks))]
pub fn assemble_native_project_resources<'py>(
    inputs: &Bound<'py, PyAny>,
    dialect: Option<String>,
    syntax_checks: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let (request, held) = project_request(inputs, dialect, syntax_checks)?;
    let resources: ProjectResources = py
        .compiler_detach(|| assemble_project_resources(&request))
        .map_err(|reason| compiler_error(native_failure(PROJECT_ASSEMBLY_CONTEXT, &reason)))?;
    resources_object(py, resources, &held)
}
