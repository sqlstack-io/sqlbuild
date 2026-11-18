//! Location publication assembles catalog data and planner-owned local identities.

use crate::bindings::_helpers::location_command::snapshot_lineage::column_lineage_document;
use crate::bindings::_helpers::location_command::snapshot_providers::providers_document;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{IntoPyObject, pyfunction};
use serde_json::{Value, json};
use sqlbuild_analysis::planner_scope::main::node_loads::node_loads;
use sqlbuild_core::models::DiagnosticSeverity;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_discovery::project_validation::main::validate_orchestration_names::validate_orchestration_names;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::orchestration_reads::main::environment_hash::environment_hash;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::command_runtime::main::published_concurrency::published_concurrency;
use sqlbuild_python_front::discovery::main::project::named_node_rows::named_node_rows;
use sqlbuild_python_front::discovery::main::project::project_facts::project_facts;
use sqlbuild_python_front::discovery::main::project::validation_error::validation_error;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use sqlbuild_python_planner::planner_identity::main::direct_model_version_identities::direct_model_version_identities;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::graph::models::ProjectSelection;
use sqlbuild_python_project::planner_identity::main::execution_deps::execution_deps;
use sqlbuild_python_project::planner_scope::main::build_planner_scope::build_planner_scope;
use sqlbuild_python_project::planner_scope::main::project_graph::project_graph;
use sqlbuild_python_project::project_snapshot::main::compile_declarations::compile_declarations;
use sqlbuild_python_project::project_snapshot::main::compile_project_roles::compile_project_roles;
use sqlbuild_python_project::project_snapshot::main::model_nodes::model_nodes;
use sqlbuild_python_project::project_snapshot::main::python_declarations::python_declarations;
use sqlbuild_python_project::project_snapshot::main::resource_nodes::resource_nodes;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationSnapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
        (&["discovered_inputs"], &[]),
    )?;
    orchestration_snapshot(
        project,
        metadata_json,
        project_dir,
        (arguments.value("discovered_inputs")?, true),
    )
}

/// Compile the snapshot from the project, its discovery and, when `declared`, its orchestration declarations.
pub(crate) fn orchestration_snapshot<'py>(
    project: PyRef<'py, NativeProjectView>,
    metadata_json: &str,
    project_dir: PathBuf,
    (discovered_inputs, declared): (Bound<'py, PyAny>, bool),
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
    input.environment_hash = environment_hash(py, &project_dir, env!("CARGO_PKG_VERSION"))?;
    let owner = project.owner().bind(py).borrow();
    let (nodes, columns) = model_nodes(py, &project, &owner, input.nodes)?;
    input.nodes = nodes;
    if !columns.is_empty() {
        input
            .documents
            .insert("columns".into(), serde_json::Value::Object(columns));
    }
    input.nodes = resource_nodes(py, &project, &owner, input.nodes)?;
    drop(owner);
    let selected =
        ProjectSelection::new((&project).into_pyobject(py)?.as_any(), &discovered_inputs)?;
    input.nodes = python_nodes(&selected, input.nodes)?;
    if let Some(providers) = providers_document(&discovered_inputs, &input.nodes)? {
        input.documents.insert("providers".into(), providers);
    }
    let graph = project.lineage_graph(py)?;
    let inventory = if declared {
        orchestration_declarations(project_dir.clone().into_pyobject(py)?.as_any())?
    } else {
        OrchestrationInventory::default()
    };
    if !inventory.is_empty() {
        let mut facts =
            project_facts(py, &discovered_inputs, named_node_rows(&discovered_inputs)?)?;
        facts.orchestration = inventory.names();
        validate_orchestration_names(&facts).map_err(|failure| validation_error(py, failure))?;
        let declarations = python_declarations(py, &project_dir, &inventory)?;
        let concurrency = published_concurrency(
            &project_dir,
            &discovered_inputs,
            project.effective_target_name().as_deref(),
        )?;
        input = compile_declarations(
            py,
            input,
            (
                (&project_dir, graph.bind(py).get().graph(), &selected),
                concurrency,
            ),
            (&inventory, declarations),
        )?;
    }
    input = compile_project_roles(
        py,
        input,
        (&project_dir, graph.bind(py).get().graph(), &selected),
    )?;
    let project = project.into_pyobject(py)?;
    if let Some(lineage) = column_lineage_document(project.as_any())? {
        input.documents.insert("lineage".into(), lineage);
    }
    input.nodes = identity_nodes(project.as_any(), input.nodes)?;
    input.nodes = load_nodes(project.as_any(), input.nodes)?;
    Ok(NativeOrchestrationSnapshot { input })
}

/// Python nodes publish their `deps` and tags, and each intermediate loader publishes the source it writes, so the server orders, selects and grants them as the CLI does.
fn python_nodes(
    selected: &ProjectSelection,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    let graph = &selected.nodes;
    let mut tags: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (tag, names) in &graph.tags {
        for name in names {
            tags.entry(name.as_str()).or_default().push(tag.as_str());
        }
    }
    let mut outputs: Vec<(String, SnapshotNode)> = Vec::new();
    for (node, (key, deps)) in graph.nodes.iter().zip(selected.node_deps()) {
        let Some(published) = nodes.get_mut(&key) else {
            return Err(PyValueError::new_err(format!(
                "Python node {key} is missing from the snapshot"
            )));
        };
        published.definition["deps"] = json!(deps);
        if let Some(sets) = selected.set_dependencies(&node.name) {
            published.definition["set_dependencies"] = sets;
        }
        published.definition["tags"] =
            json!(tags.get(node.name.as_str()).cloned().unwrap_or_default());
        if let Some(source) = selected.intermediate_source(&node.kind, &node.name) {
            let mut definition = json!({
                "name": node.name,
                "entry": {"loader": node.name},
                "intermediate_loader": true,
                "deps": [key],
            });
            if let Some(path) = published.definition.get("path") {
                definition["path"] = path.clone();
            }
            outputs.push((
                source,
                SnapshotNode {
                    kind: "source".into(),
                    definition,
                    raw_sql: None,
                    compiled_sql: None,
                    python_source: None,
                },
            ));
        }
    }
    for (key, node) in outputs {
        if nodes.insert(key.clone(), node).is_some() {
            return Err(PyValueError::new_err(format!(
                "intermediate loader {key} collides with a published node"
            )));
        }
    }
    Ok(nodes)
}

fn identity_nodes(
    project: &Bound<'_, PyAny>,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    let py = project.py();
    let options = PyDict::new(py);
    options.set_item("project", project)?;
    let scope = build_planner_scope(py, Some(options))?;
    let dialect: Option<String> = field(project, "sql_analysis_dialect")?.extract()?;
    let identities = direct_model_version_identities(
        (&field(project, "functions")?, &scope),
        dialect.clone(),
        (
            &field(project, "seeds")?,
            &py.None().into_bound(py),
            &field(project, "hook_functions")?,
        ),
        &py.None().into_bound(py),
    )?;
    for node in nodes.values_mut() {
        let (hashes, with_dialect) = match node.kind.as_str() {
            "model" => ("model_local_hashes", true),
            "udf" | "table_fn" => ("function_local_hashes", false),
            "seed" => ("seed_version_hashes", false),
            _ => continue,
        };
        let name = node.definition["name"].as_str().unwrap_or_default();
        let hash: String = field(&identities, hashes)?.get_item(name)?.extract()?;
        node.definition["planner_identity"] = if with_dialect {
            json!({"local_hash": hash, "dialect": dialect})
        } else {
            json!({"local_hash": hash})
        };
    }
    Ok(nodes)
}

/// `loads` lists the source writers a loading build of the node plans beyond its compiled `deps`, absent when none.
fn load_nodes(
    project: &Bound<'_, PyAny>,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    for ((kind, name), writers) in node_loads(&project_graph(project)?, &execution_deps(project)?) {
        let Some(node) = nodes.get_mut(&format!("{kind}:{name}")) else {
            continue;
        };
        let deps: Vec<Value> = node
            .definition
            .get("deps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let loads: Vec<String> = writers
            .into_iter()
            .map(|(kind, name)| format!("{kind}:{name}"))
            .filter(|key| !listed(&deps, key))
            .collect();
        if !loads.is_empty() {
            node.definition["loads"] = json!(loads);
        }
    }
    Ok(nodes)
}

/// Whether `key` is already one of the node's compiled `deps`.
fn listed(deps: &[Value], key: &str) -> bool {
    deps.iter().any(|dep| dep.as_str() == Some(key))
}
