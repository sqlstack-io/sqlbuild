//! Build the static dependency graph for a compiled project.

use crate::compile_driver::_helpers::setup::project::GRAPH_OPTIONAL;
use crate::compile_driver::_helpers::setup::project::py_bool;
use crate::compile_driver::_helpers::setup::project::selected_graph;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::compile_driver::constants::PROJECT_GRAPHS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Build the static dependency graph for a compiled project.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_project_graph<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    count_stage(py, PROJECT_GRAPHS, 1)?;
    let mut optional: Vec<&str> = GRAPH_OPTIONAL.to_vec();
    optional.push("no_cache");
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_project_graph",
        options,
        (&["discovered_inputs", "adapter"], &optional),
    )?;
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompileAnalysisSelection,
        &[("no_cache", py_bool(py, arguments.flag("no_cache")?))],
    )?;
    selected_graph(&arguments, selection)
}
