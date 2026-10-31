//! Project metadata stays native until its immutable snapshot is sent over HTTP.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use serde_json::Value;
use sqlbuild_core::models::DiagnosticSeverity;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode, SnapshotRecord};

use crate::bindings::_helpers::compiled_project::snapshot_declarations::{
    AuthoredDeclaration, compile_declarations, discover_declarations,
};
use crate::bindings::_helpers::compiled_project::snapshot_models::model_nodes;
use crate::bindings::_helpers::compiled_project::snapshot_resources::resource_nodes;
use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::native_host::orchestration_declarations::collect_declarations;
use crate::bindings::_helpers::native_host::orchestration_environment::environment_hash;
use crate::bindings::_helpers::records::fields::field;

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

/// Compile catalog nodes from their owners, never from Python model or graph projections.
#[pyfunction]
#[pyo3(signature = (project, metadata_json, project_dir, *, declarations=None))]
pub(crate) fn build_orchestration_snapshot(
    project: PyRef<'_, NativeProjectView>,
    metadata_json: &str,
    project_dir: PathBuf,
    declarations: Option<Vec<Bound<'_, PyAny>>>,
) -> PyResult<NativeOrchestrationSnapshot> {
    let py = project.py();
    let mut input: SnapshotInput = serde_json::from_str(metadata_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    if !input.nodes.is_empty() {
        return Err(PyValueError::new_err(
            "snapshot nodes must come from the compiled project",
        ));
    }
    if project
        .effective_target_name()
        .is_some_and(|target| target != input.target)
    {
        return Err(PyValueError::new_err(
            "snapshot target differs from the compiled project",
        ));
    }
    for diagnostic in project.diagnostics(py).bind(py).try_iter()? {
        let severity: String = field(&field(&diagnostic?, "severity")?, "value")?.extract()?;
        if severity == DiagnosticSeverity::Error.as_str() {
            return Err(PyValueError::new_err(
                "cannot publish a project with compiler errors",
            ));
        }
    }
    input.sqb_version = env!("CARGO_PKG_VERSION").into();
    input.environment_hash = environment_hash(py, &project_dir)?;
    let owner = project.owner().bind(py).borrow();
    input.nodes = model_nodes(py, &project, &owner, input.nodes)?;
    input.nodes = resource_nodes(py, &project, &owner, input.nodes)?;
    drop(owner);
    let declarations = match declarations {
        Some(declarations) => declarations,
        None => discover_declarations(py, &project_dir)?,
    };
    if !declarations.is_empty() {
        let declarations = collect_declarations(py, declarations)?;
        let authored = declarations
            .iter()
            .map(|declaration| declaration.extract::<AuthoredDeclaration>())
            .collect::<PyResult<Vec<AuthoredDeclaration>>>()?;
        let graph = project.lineage_graph(py)?;
        input = compile_declarations(input, graph.bind(py).get().graph(), authored)?;
    }
    Ok(NativeOrchestrationSnapshot { input })
}

/// Duplicate resource identities must fail, rather than silently replacing an earlier node.
pub(crate) fn insert_node(
    mut nodes: BTreeMap<String, SnapshotNode>,
    key: String,
    node: SnapshotNode,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    if nodes.insert(key, node).is_some() {
        return Err(PyValueError::new_err("duplicate snapshot node identity"));
    }
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
    module.add_class::<NativeOrchestrationWorker>()?;
    module.add_function(wrap_pyfunction!(build_orchestration_snapshot, module)?)
}
