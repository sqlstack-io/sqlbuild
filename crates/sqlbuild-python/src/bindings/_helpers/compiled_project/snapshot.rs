//! Project metadata stays native until its immutable snapshot is sent over HTTP.

use std::collections::BTreeMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyclass, pymethods};
use serde_json::Value;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode, SnapshotRecord};

use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;

/// A data-only publication; this object never contains adapters, connections or user callables.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct NativeOrchestrationSnapshot {
    pub(crate) input: SnapshotInput,
}

impl NativeOrchestrationSnapshot {
    /// Native callers retain transport errors so they can report an uncertain acknowledgement.
    pub(crate) fn publication(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
    ) -> PyResult<Result<SnapshotRecord, ClientError>> {
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let result = runtime.block_on(publish_remote_snapshot(server_url, bearer, &self.input));
            runtime.shutdown_timeout(std::time::Duration::from_secs(1));
            Ok(result)
        })
    }
}

#[pymethods]
impl NativeOrchestrationSnapshot {
    /// Serialize only at the public boundary; native transports can use the retained input.
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| serde_json::to_string(&self.input))
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Publish retained data without projecting it through Python, releasing the GIL during IO.
    fn publish(&self, py: Python<'_>, server_url: &str, bearer: &str) -> PyResult<String> {
        let record = self
            .publication(py, server_url, bearer)?
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        py.detach(|| {
            serde_json::to_string(&record)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Retain the verified claim and its independent native heartbeat in the worker interpreter.
    fn start_worker(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
        identity: (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        NativeOrchestrationWorker::start(self, py, (server_url, bearer), identity)
    }
}

/// Duplicate resource identities must fail, rather than silently replacing an earlier node.
pub(crate) fn insert_node(
    mut nodes: BTreeMap<String, SnapshotNode>,
    key: String,
    node: SnapshotNode,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    if nodes.contains_key(&key) {
        return Err(PyValueError::new_err(format!(
            "duplicate snapshot node identity {key}"
        )));
    }
    nodes.insert(key, node);
    Ok(nodes)
}

pub(crate) fn dependency_keys(deps: &[(String, String)]) -> Vec<String> {
    deps.iter()
        .map(|(kind, name)| format!("{kind}:{name}"))
        .collect()
}

pub(crate) fn destination(value: &sqlbuild_project::models::DestinationFact) -> Value {
    serde_json::json!({
        "database": value.database,
        "schema": value.schema,
        "name": value.name,
        "qualified_name": value.qualified_name,
        "logical_database": value.logical_database,
        "logical_schema": value.logical_schema,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeOrchestrationSnapshot>()?;
    module.add_class::<NativeOrchestrationWorker>()
}
