//! `build_dag_json`: the static DAG artifact of a compiled project graph.

use pyo3::exceptions::{PyKeyError, PyRecursionError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::dag_artifact::errors::DagArtifactError;
use sqlbuild_analysis::dag_artifact::main::build_dag_json::build_dag_json as build;
use sqlbuild_analysis::dag_artifact::models::{DagProjectInput, DagPythonGraphInput};
use sqlbuild_core::json::errors::JsonEmitError;

use crate::bindings::_helpers::dag_artifact::rows::{project_input, python_graph_input};
use crate::bindings::_helpers::graph::project_graph::CompiledProjectGraph;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

fn python_error(error: DagArtifactError, py: pyo3::Python<'_>) -> PyErr {
    match error {
        DagArtifactError::LoaderDestination(message) => error_class(py, ErrorId::SpecConfig)
            .and_then(|class| class.call1((message,)))
            .map_or_else(|error| error, PyErr::from_value),
        DagArtifactError::MissingParameterType(name) => PyKeyError::new_err(name),
        DagArtifactError::Unencodable(message) => PyTypeError::new_err(message),
        DagArtifactError::Emit(JsonEmitError::NestingTooDeep) => {
            PyRecursionError::new_err("maximum recursion depth exceeded")
        }
        DagArtifactError::Emit(error) => PyValueError::new_err(format!("{error:?}")),
    }
}

/// The DAG JSON of `graph` (a `ProjectGraph`) and the optional discovered Python node graph.
#[pyfunction]
#[pyo3(signature = (*, graph, project_name, python_graph=None))]
pub(crate) fn build_dag_json(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_dag_json, module)?)
}
