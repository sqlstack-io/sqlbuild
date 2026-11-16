//! A new, empty recorder of one compile's artifact writes.

use crate::compile_command::models::CompileArtifactWrites;
use pyo3::prelude::{Py, PyResult, Python};
use std::collections::HashMap;
use std::sync::Mutex;

/// A new, empty recorder of one compile's artifact writes.
pub fn new_artifact_recorder(py: Python<'_>) -> PyResult<Py<CompileArtifactWrites>> {
    Py::new(
        py,
        CompileArtifactWrites {
            artifacts: Mutex::new(HashMap::new()),
        },
    )
}
