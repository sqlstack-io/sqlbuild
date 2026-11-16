//! Build a static dependency graph for an already-compiled project.

use crate::compile_driver::main::setup::project_graph::project_graph;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Build a static dependency graph for an already-compiled project.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_project_graph_from_compiled_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_project_graph_from_compiled_project",
        options,
        (&["project"], &[]),
    )?;
    project_graph(&arguments.value("project")?)
}
