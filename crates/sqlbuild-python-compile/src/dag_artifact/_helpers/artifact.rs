//! `build_dag_json`: the static DAG artifact of a compiled project graph.

use pyo3::exceptions::{PyKeyError, PyRecursionError, PyTypeError, PyValueError};
use pyo3::prelude::{PyAnyMethods, PyErr};
use sqlbuild_analysis::dag_artifact::errors::DagArtifactError;
use sqlbuild_core::json::errors::JsonEmitError;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) fn python_error(error: DagArtifactError, py: pyo3::Python<'_>) -> PyErr {
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
