//! Location publication assembles catalog data and planner-owned local identities.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{IntoPyObject, pyfunction};
use serde_json::json;
use sqlbuild_core::models::DiagnosticSeverity;
use sqlbuild_discovery::project_validation::main::validate_orchestration_names::validate_orchestration_names;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_project::snapshot::NativeOrchestrationSnapshot;
use crate::bindings::_helpers::compiled_project::snapshot_declarations::{
    compile_declarations, discover_declarations,
};
use crate::bindings::_helpers::compiled_project::snapshot_models::model_nodes;
use crate::bindings::_helpers::compiled_project::snapshot_resources::resource_nodes;
use crate::bindings::_helpers::compiled_project::snapshot_roles::compile_project_roles;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::discovery::project_facts::{named_node_rows, project_facts};
use crate::bindings::_helpers::discovery::project_validation::validation_error;
use crate::bindings::_helpers::location_command::snapshot_lineage::column_lineage_document;
use crate::bindings::_helpers::native_host::orchestration_declarations::{
    collect_declarations, orchestration_names,
};
use crate::bindings::_helpers::native_host::orchestration_environment::environment_hash;
use crate::bindings::_helpers::planner_identity::identities::direct_model_version_identities;
use crate::bindings::_helpers::planner_scope::scope::build_planner_scope;
use crate::bindings::_helpers::records::fields::field;

/// Where declarations passed to the snapshot directly are reported as declared.
const DECLARATIONS_ARGUMENT: &str = "declarations argument";

/// Compile catalog nodes and identities in the location, never in the server.
#[pyfunction]
#[pyo3(signature = (project, metadata_json, project_dir, **options))]
pub(crate) fn build_orchestration_snapshot<'py>(
    project: PyRef<'py, NativeProjectView>,
    metadata_json: &str,
    project_dir: PathBuf,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<NativeOrchestrationSnapshot> {
    let arguments = KeywordArguments::read(
        project.py(),
        "build_orchestration_snapshot",
        options,
        (&["discovered_inputs"], &["declarations"]),
    )?;
    let declarations = arguments.value("declarations")?;
    orchestration_snapshot(
        project,
        metadata_json,
        project_dir,
        (
            arguments.value("discovered_inputs")?,
            (!declarations.is_none())
                .then(|| declarations.extract())
                .transpose()?,
        ),
    )
}

/// Compile the snapshot from the project, its discovery, and the declarations passed directly, if any.
pub(crate) fn orchestration_snapshot<'py>(
    project: PyRef<'py, NativeProjectView>,
    metadata_json: &str,
    project_dir: PathBuf,
    (discovered_inputs, declarations): (Bound<'py, PyAny>, Option<Vec<Bound<'py, PyAny>>>),
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
        Some(declarations) => declarations
            .into_iter()
            .map(|declaration| (declaration, DECLARATIONS_ARGUMENT.to_owned()))
            .collect(),
        None => discover_declarations(py, &project_dir)?,
    };
    let graph = project.lineage_graph(py)?;
    if !declarations.is_empty() {
        let mut facts =
            project_facts(py, &discovered_inputs, named_node_rows(&discovered_inputs)?)?;
        facts.orchestration = orchestration_names(py, &declarations)?;
        validate_orchestration_names(&facts).map_err(|failure| validation_error(py, failure))?;
        let authored = collect_declarations(
            py,
            declarations
                .into_iter()
                .map(|(declaration, _)| declaration)
                .collect(),
        )?;
        input = compile_declarations(
            py,
            input,
            (&project_dir, graph.bind(py).get().graph()),
            authored,
        )?;
    }
    input = compile_project_roles(py, input, (&project_dir, graph.bind(py).get().graph()))?;
    let project = project.into_pyobject(py)?;
    if let Some(lineage) = column_lineage_document(project.as_any())? {
        input.documents.insert("lineage".into(), lineage);
    }
    input.nodes = identity_nodes(project.as_any(), input.nodes)?;
    Ok(NativeOrchestrationSnapshot { input })
}

fn identity_nodes(
    project: &Bound<'_, PyAny>,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    let py = project.py();
    let options = PyDict::new(py);
    options.set_item("project", project)?;
    let scope = build_planner_scope(py, Some(options))?;
    let identities = direct_model_version_identities(
        (&field(project, "functions")?, &scope),
        field(project, "sql_analysis_dialect")?.extract()?,
        (
            &field(project, "seeds")?,
            &py.None().into_bound(py),
            &field(project, "hook_functions")?,
        ),
    )?;
    for node in nodes.values_mut() {
        let hashes = match node.kind.as_str() {
            "model" => "model_local_hashes",
            "udf" | "table_fn" => "function_local_hashes",
            "seed" => "seed_version_hashes",
            _ => continue,
        };
        let name = node.definition["name"].as_str().unwrap_or_default();
        let hash: String = field(&identities, hashes)?.get_item(name)?.extract()?;
        node.definition["planner_identity"] = json!({"local_hash": hash});
    }
    Ok(nodes)
}
