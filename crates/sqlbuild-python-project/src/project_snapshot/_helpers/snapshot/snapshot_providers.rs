//! The snapshot nodes that run a provider's consumers.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::{Value, json};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;
use sqlbuild_python_base::orchestration_reads::main::missing_secrets::missing_secrets;
use sqlbuild_python_base::orchestration_reads::main::provider_class_settings::provider_class_settings;
use sqlbuild_python_base::orchestration_reads::main::setting_rows::setting_rows;
use sqlbuild_python_base::records::main::fields::field::field;

use crate::project_snapshot::_helpers::snapshot::relative_path::relative_path;
use crate::project_snapshot::constants::{
    CONSUMERS, MODEL_NODE_KIND, SNAPSHOT_HIDDEN_SETTING, SOURCE_NODE_KIND,
};
use crate::project_snapshot::models::RunsIn;
use crate::project_snapshot::types::ProviderConsumers;

/// Each provider's consumers, as `{kind, name}`, and the snapshot nodes that run them.
pub(crate) fn provider_consumers(
    discovered_inputs: &Bound<'_, PyAny>,
    nodes: &BTreeMap<String, SnapshotNode>,
) -> PyResult<ProviderConsumers> {
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
    Ok((used_by, running))
}

/// One provider's published row: its class, file and settings, each flagged when its variable is unset; never a value.
pub(crate) fn provider_row(
    provider: &Bound<'_, PyAny>,
    nodes: BTreeSet<String>,
    used_by: Vec<Value>,
) -> PyResult<Value> {
    let name: String = field(provider, "name")?.extract()?;
    let provider_class = field(provider, "provider_class")?;
    let class = provider_class_settings(&provider_class)?;
    let missing: Vec<String> = missing_secrets(&provider_class)?;
    let mut settings: Vec<Value> = setting_rows(&class);
    for setting in &mut settings {
        let unset = setting["env"]
            .as_str()
            .is_some_and(|env| missing.iter().any(|name| name == env));
        setting["missing"] = Value::Bool(unset);
        if let Some(row) = setting.as_object_mut() {
            row.remove(SNAPSHOT_HIDDEN_SETTING);
        }
    }
    Ok(json!({
        "name": name,
        "class": class["class"],
        "description": class["description"],
        "path": relative_path(provider)?,
        "settings": settings,
        "nodes": nodes,
        "used_by": used_by,
    }))
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
