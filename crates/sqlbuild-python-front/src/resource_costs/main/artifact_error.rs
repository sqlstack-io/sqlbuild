//! The `CostArtifactError` Python raised for a cost artifact.

use pyo3::prelude::{PyAnyMethods, PyErr, Python};
use sqlbuild_core::cost_report::errors::CostArtifactError;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

/// The `CostArtifactError` Python raised for the same artifact.
pub fn artifact_error(py: Python<'_>, error: &CostArtifactError) -> PyErr {
    match native_attr(py, NativeAttr::CostArtifactError)
        .and_then(|class| class.call1((error.message.as_str(),)))
    {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}
