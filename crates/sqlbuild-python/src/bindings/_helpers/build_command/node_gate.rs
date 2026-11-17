//! A group member that waits per node asks before its build runs each gated node.

use std::sync::Arc;

use pyo3::prelude::{PyResult, Python};
use pyo3::{pyclass, pymethods};

use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;

/// The build scheduler's node gate for one worker build.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NodeGateCallback {
    pub(crate) worker: Arc<WorkerBuild>,
}

#[pymethods]
impl NodeGateCallback {
    /// Each of `keys` that may not run yet, with its failed input when it is skipped, or none while it waits.
    fn __call__(
        &self,
        py: Python<'_>,
        keys: Vec<String>,
    ) -> PyResult<Vec<(String, Option<String>)>> {
        self.worker.gate(py, &keys)
    }
}
