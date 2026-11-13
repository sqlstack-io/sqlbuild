//! The project's providers and what uses them, published from the classes alone, so setting values never leave the location.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::{Value, json};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_project::snapshot_boundary::relative_path;
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::discovery::imported_modules::shim;
use crate::bindings::_helpers::location_command::constants::{MODEL_NODE_KIND, SOURCE_NODE_KIND};
use crate::bindings::_helpers::records::fields::field;

/// How a consumer's code runs inside other snapshot nodes, besides its own node.
#[derive(Clone, Copy)]
enum RunsIn {
    Itself,
    ModelHooks,
    ModelMaterialization,
    SourceLoader,
}

/// Discovered collections whose items inject providers, the kind each is published as, and where it runs.
const CONSUMERS: [(&str, &str, RunsIn); 8] = [
    ("loader", "loader_functions", RunsIn::SourceLoader),
    ("task", "task_functions", RunsIn::Itself),
    ("asset", "asset_functions", RunsIn::Itself),
    ("check", "check_functions", RunsIn::Itself),
    ("hook", "hook_functions", RunsIn::ModelHooks),
    (
        "materialization",
        "materialization_files",
        RunsIn::ModelMaterialization,
    ),
    ("event_exporter", "event_exporters", RunsIn::Itself),
    ("sink", "command_output_sinks", RunsIn::Itself),
];

/// Each provider's name, class, description, file, setting fields, consumers and the snapshot nodes that run them; `None` when the project declares none.
pub(crate) fn providers_document(
    discovered_inputs: &Bound<'_, PyAny>,
    nodes: &BTreeMap<String, SnapshotNode>,
) -> PyResult<Option<Value>> {
    let py = discovered_inputs.py();
    let providers = field(discovered_inputs, "providers")?;
    if providers.len()? == 0 {
        return Ok(None);
    }
    let mut used_by: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut running: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (kind, collection, runs_in) in CONSUMERS {
        for consumer in field(discovered_inputs, collection)?.try_iter()? {
            let consumer = consumer?;
            let name: String = field(&consumer, "name")?.extract()?;
            let keys = running_nodes(nodes, (kind, &name, runs_in));
            for usage in field(&consumer, "provider_usages")?.try_iter()? {
                let provider: String = field(&usage?, "provider_name")?.extract()?;
                used_by
                    .entry(provider.clone())
                    .or_default()
                    .push(json!({ "kind": kind, "name": name }));
                running.entry(provider).or_default().extend(keys.clone());
            }
        }
    }
    let read_class = shim(py, UserCodeHost::ProviderSettings)?;
    let mut rows: Vec<Value> = Vec::new();
    for provider in providers.try_iter()? {
        let provider = provider?;
        let name: String = field(&provider, "name")?.extract()?;
        let class = plain_value(&call_with(
            &read_class,
            &[("provider_class", field(&provider, "provider_class")?)],
        )?)?;
        rows.push(json!({
            "name": name,
            "class": class["class"],
            "description": class["description"],
            "path": relative_path(&provider)?,
            "settings": settings(&class),
            "nodes": running.remove(&name).unwrap_or_default(),
            "used_by": used_by.remove(&name).unwrap_or_default(),
        }));
    }
    Ok(Some(json!({ "providers": rows })))
}

/// Snapshot nodes that run one consumer: its own node, models calling a hook or using a materialization, and sources a loader fills.
fn running_nodes(
    nodes: &BTreeMap<String, SnapshotNode>,
    (kind, name, runs_in): (&str, &str, RunsIn),
) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = BTreeSet::new();
    let own = format!("{kind}:{name}");
    if nodes.contains_key(&own) {
        keys.insert(own);
    }
    for (key, node) in nodes {
        let definition = &node.definition;
        let runs = match runs_in {
            RunsIn::Itself => false,
            RunsIn::ModelHooks => calls_hook(definition, name),
            RunsIn::ModelMaterialization => {
                node.kind == MODEL_NODE_KIND && definition["materialization"].as_str() == Some(name)
            }
            RunsIn::SourceLoader => {
                node.kind == SOURCE_NODE_KIND
                    && definition["entry"]["loader"].as_str() == Some(name)
            }
        };
        if runs {
            keys.insert(key.clone());
        }
    }
    keys
}

/// Whether a model's pre or post hooks call the hook, written as its name or as `{"name": ...}`.
fn calls_hook(definition: &Value, hook: &str) -> bool {
    for hooks in ["pre_hooks", "post_hooks"] {
        let Some(calls) = definition["config"][hooks].as_array() else {
            continue;
        };
        for call in calls {
            if call.as_str() == Some(hook) || call["name"].as_str() == Some(hook) {
                return true;
            }
        }
    }
    false
}

/// Declared fields with the environment variable each is read from, which pydantic-settings derives from the prefix unless an alias names it.
fn settings(class: &Value) -> Vec<Value> {
    let prefix = class["env_prefix"].as_str().unwrap_or_default();
    let case_sensitive = class["case_sensitive"].as_bool().unwrap_or_default();
    let mut rows: Vec<Value> = Vec::new();
    for field in class["fields"].as_array().into_iter().flatten() {
        let name = field["name"].as_str().unwrap_or_default();
        let type_name = field["type"].as_str().unwrap_or_default();
        let env = field["alias"]
            .as_str()
            .map_or_else(|| format!("{prefix}{name}"), str::to_owned);
        rows.push(json!({
            "name": name,
            "type": type_name,
            "required": field["required"],
            "secret": type_name.contains("Secret"),
            "env": if case_sensitive { env } else { env.to_uppercase() },
            "description": field["description"],
        }));
    }
    rows
}
