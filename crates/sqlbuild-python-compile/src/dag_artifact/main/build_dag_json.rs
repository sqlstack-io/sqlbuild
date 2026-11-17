//! The DAG JSON of `graph` (a `ProjectGraph`) and the optional discovered Python node graph.

use crate::dag_artifact::_helpers::artifact::python_error;
use crate::dag_artifact::_helpers::rows::project_input;
use crate::dag_artifact::_helpers::rows::python_graph_input;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_analysis::dag_artifact::main::build_dag_json::build_dag_json as build;
use sqlbuild_analysis::dag_artifact::models::{DagProjectInput, DagPythonGraphInput};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_project::graph::models::CompiledProjectGraph;

/// The DAG JSON of `graph` (a `ProjectGraph`) and the optional discovered Python node graph.
#[pyfunction]
#[pyo3(signature = (*, graph, project_name, python_graph=None))]
pub fn build_dag_json(
    graph: &Bound<'_, PyAny>,
    project_name: &str,
    python_graph: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    compiler_guard(|| {
        let py = graph.py();
        let graph: &Bound<'_, CompiledProjectGraph> = graph.cast()?;
        let project: DagProjectInput = project_input(graph.get().project(py))?;
        let python: Option<DagPythonGraphInput> = python_graph
            .filter(|python_graph| !python_graph.is_none())
            .map(python_graph_input)
            .transpose()?;
        build(
            graph.get().native(py).get().graph(),
            &project,
            python.as_ref(),
            project_name,
        )
        .map_err(|error| python_error(error, py))
    })
}
