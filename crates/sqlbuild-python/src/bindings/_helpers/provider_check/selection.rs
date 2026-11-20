//! The selected nodes that use a provider missing secrets, and the selected nodes that read them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::orchestration_reads::main::missing_secrets::missing_secrets;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::discovery::main::sources::sink_uses::sink_uses;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::graph::models::ProjectSelection;

/// Discovered Python nodes that run as their own node, with the key kind each publishes.
const OWN_NODES: [(&str, &str); 3] = [
    ("task_functions", "task"),
    ("asset_functions", "asset"),
    ("check_functions", "check"),
];

/// One provider whose missing secrets stop part of a selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SecretBlock {
    pub(crate) provider: String,
    /// The provider file, relative to the project.
    pub(crate) path: String,
    /// The unset variables, sorted.
    pub(crate) variables: Vec<String>,
    /// Selected node keys that run the provider, such as `source:raw_payments`; sinks as `sink:<name>`.
    pub(crate) users: Vec<String>,
    /// Selected node keys downstream of the users, which can't run without them.
    pub(crate) readers: Vec<String>,
}

/// Providers whose required secrets are unset, by name, with their file and variables.
fn missing_providers(
    discovered: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, String, Vec<String>)>> {
    let mut missing: Vec<(String, String, Vec<String>)> = Vec::new();
    for provider in field(discovered, "providers")?.try_iter()? {
        let provider = provider?;
        let variables: Vec<String> = missing_secrets(&field(&provider, "provider_class")?)?;
        if !variables.is_empty() {
            missing.push((
                field(&provider, "name")?.extract()?,
                field(&provider, "relative_path")?.str()?.extract()?,
                variables,
            ));
        }
    }
    Ok(missing)
}

/// The names of `collection`'s items that inject `provider`.
fn consumers_of(
    discovered: &Bound<'_, PyAny>,
    collection: &str,
    provider: &str,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    let Some(items) = discovered.getattr_opt(collection)? else {
        return Ok(names);
    };
    for item in items.try_iter()? {
        let item = item?;
        for usage in field(&item, "provider_usages")?.try_iter()? {
            if field(&usage?, "provider_name")?.extract::<String>()? == provider {
                names.push(field(&item, "name")?.extract()?);
                break;
            }
        }
    }
    Ok(names)
}

/// Whether a hook list calls `hook`, written as its name or as `{"name": ...}`.
fn calls(hooks: &Bound<'_, PyAny>, hook: &str) -> PyResult<bool> {
    if hooks.is_none() || hooks.is_instance_of::<PyString>() {
        return Ok(hooks.extract::<String>().is_ok_and(|name| name == hook));
    }
    let Ok(items) = hooks.try_iter() else {
        return Ok(false);
    };
    for item in items {
        let item = item?;
        let name: Option<String> = if item.is_instance_of::<PyString>() {
            item.extract().ok()
        } else {
            item.get_item("name")
                .ok()
                .and_then(|name| name.extract().ok())
        };
        if name.as_deref() == Some(hook) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `model:<name>` of each compiled model whose hooks call one of `hooks` or that a custom `materializations` builds.
fn model_users(
    project: &Bound<'_, PyAny>,
    (hooks, materializations): (&[String], &[String]),
) -> PyResult<Vec<String>> {
    let py: Python<'_> = project.py();
    let mut keys: Vec<String> = Vec::new();
    if hooks.is_empty() && materializations.is_empty() {
        return Ok(keys);
    }
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        let values = model_values(&model)?;
        let mut uses: bool = false;
        for key in ["pre_hooks", "post_hooks"] {
            let called = config_value(py, &values, key)?;
            for hook in hooks {
                uses |= calls(&called, hook)?;
            }
        }
        if let Ok(materialized) = config_value(py, &values, "materialized")?.extract::<String>() {
            uses |= materializations.contains(&materialized);
        }
        if uses {
            keys.push(format!("model:{}", model_name(&model)?));
        }
    }
    Ok(keys)
}

/// Every node key that runs `provider`: its Python nodes, the sources its loaders fill, models calling its hooks or materializations, and its sinks.
fn provider_users(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selection: &ProjectSelection,
    provider: &str,
) -> PyResult<BTreeSet<String>> {
    let mut users: BTreeSet<String> = BTreeSet::new();
    for (collection, kind) in OWN_NODES {
        for name in consumers_of(discovered, collection, provider)? {
            users.insert(
                selection
                    .node_key(&name)
                    .unwrap_or(format!("{kind}:{name}")),
            );
        }
    }
    for name in consumers_of(discovered, "loader_functions", provider)? {
        users.extend(selection.run_key(&name));
    }
    let hooks = consumers_of(discovered, "hook_functions", provider)?;
    let materializations = consumers_of(discovered, "materialization_files", provider)?;
    users.extend(model_users(project, (&hooks, &materializations))?);
    Ok(users)
}

/// Each key's direct readers: SQL lineage edges and Python nodes' upstream keys, inverted.
fn readers_by_key(
    graph: &sqlbuild_analysis::graph::models::ProjectGraph,
    selection: &ProjectSelection,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut readers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ((kind, name), downstream) in &graph.downstream {
        readers.entry(format!("{kind}:{name}")).or_default().extend(
            downstream
                .iter()
                .map(|(kind, name)| format!("{kind}:{name}")),
        );
    }
    for (node, deps) in selection.node_deps() {
        for dep in deps {
            readers.entry(dep).or_default().insert(node.clone());
        }
    }
    readers
}

/// Selected keys downstream of `users`, walking only through selected nodes.
fn downstream_readers(
    users: &BTreeSet<String>,
    selected: &BTreeSet<String>,
    readers: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut pending: VecDeque<String> = users.iter().cloned().collect();
    while let Some(key) = pending.pop_front() {
        for reader in readers.get(&key).into_iter().flatten() {
            if selected.contains(reader) && !users.contains(reader) && found.insert(reader.clone())
            {
                pending.push_back(reader.clone());
            }
        }
    }
    found.into_iter().collect()
}

/// The providers missing secrets that `select`/`exclude` would run, with the nodes they stop; empty when nothing is missing.
///
/// Reads only the compiled project and the providers' classes, never the warehouse, so a run can stop before connecting.
pub(crate) fn selection_secret_blocks(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (select, exclude): (&[String], &[String]),
) -> PyResult<Vec<SecretBlock>> {
    if missing_providers(discovered)?.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = project.py();
    let view: PyRef<'_, NativeProjectView> = project.extract()?;
    let graph = view.lineage_graph(py)?;
    let selection = ProjectSelection::new(project, discovered)?;
    let selected: BTreeSet<String> = selection
        .run_keys(graph.bind(py).get().graph(), select, exclude)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    drop(view);
    key_secret_blocks((discovered, project), &selected)
}

/// The providers missing secrets that the run of exactly `selected` node keys uses, with the nodes they stop.
pub(crate) fn key_secret_blocks(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selected: &BTreeSet<String>,
) -> PyResult<Vec<SecretBlock>> {
    let missing = missing_providers(discovered)?;
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = project.py();
    let view: PyRef<'_, NativeProjectView> = project.extract()?;
    let graph = view.lineage_graph(py)?;
    let graph = graph.bind(py).get().graph();
    let selection = ProjectSelection::new(project, discovered)?;
    let readers = readers_by_key(graph, &selection);
    let sinks: Vec<(String, String)> = sink_uses(discovered)?;
    let mut blocks: Vec<SecretBlock> = Vec::new();
    for (provider, path, variables) in missing {
        let mut users: BTreeSet<String> =
            provider_users((discovered, project), &selection, &provider)?
                .into_iter()
                .filter(|key| selected.contains(key))
                .collect();
        users.extend(
            sinks
                .iter()
                .filter(|(used, _)| *used == provider)
                .map(|(_, sink)| format!("sink:{sink}")),
        );
        if users.is_empty() {
            continue;
        }
        blocks.push(SecretBlock {
            readers: downstream_readers(&users, selected, &readers),
            users: users.into_iter().collect(),
            provider,
            path,
            variables,
        });
    }
    Ok(blocks)
}

/// The plan JSON's `cant_run` section: each provider, its unset variables, the nodes that use it and those that read them.
pub(crate) fn cant_run_json(blocks: &[SecretBlock], env_file: Option<&str>) -> serde_json::Value {
    serde_json::Value::Array(
        blocks
            .iter()
            .map(|block| {
                serde_json::json!({
                    "reason": "missing_secret",
                    "provider": block.provider,
                    "path": block.path,
                    "variables": block.variables,
                    "nodes": block.users,
                    "readers": block.readers,
                    "env_file": env_file,
                })
            })
            .collect(),
    )
}
