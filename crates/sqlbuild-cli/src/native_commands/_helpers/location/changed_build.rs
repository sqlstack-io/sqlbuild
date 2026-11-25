//! What a location's "Build what changed" plan selects, and the settings it plans under.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::adhoc::main::command_settings::command_settings;
use crate::native_commands::constants::CHANGED_BUILD_COMMAND;

/// The settings the server freezes a plan request of `sqb build --changed` to; with no selector the plan selects every node, as the frozen selection does.
pub(in crate::native_commands) fn changed_build_settings() -> Result<BTreeMap<String, Value>, String>
{
    command_settings(&CHANGED_BUILD_COMMAND.map(str::to_owned))
}

/// The nodes a `sqb plan --json --changed` document runs: for a change of their own, and only because an upstream runs; or why it has none.
pub(in crate::native_commands) fn changed_build_nodes(
    document: &Value,
) -> Result<(Vec<String>, Vec<String>), String> {
    if let Some(message) = document["error"]["message"].as_str() {
        return Err(message.to_owned());
    }
    let runs = document["changed"]["runs"]
        .as_array()
        .ok_or_else(|| "the plan has no --changed section".to_owned())?;
    let mut changed: Vec<String> = Vec::new();
    let mut downstream: Vec<String> = Vec::new();
    for run in runs {
        let node = run["node"]
            .as_str()
            .ok_or_else(|| "a planned run names no node".to_owned())?
            .to_owned();
        if run["downstream_of"].is_null() {
            changed.push(node);
        } else {
            downstream.push(node);
        }
    }
    Ok((changed, downstream))
}

/// `Can't plan N models: name: error; ...` for the plan's models that couldn't be planned; `None` when every model planned.
pub(in crate::native_commands) fn model_errors(document: &Value) -> Option<String> {
    let mut errors: Vec<String> = Vec::new();
    for model in document["models"].as_array().into_iter().flatten() {
        let Some(message) = model["error"]["message"].as_str() else {
            continue;
        };
        let name: &str = model["name"].as_str().unwrap_or("a model");
        errors.push(format!("{name}: {message}"));
    }
    match errors.len() {
        0 => None,
        1 => Some(format!("Can't plan 1 model: {}", errors[0])),
        count => Some(format!("Can't plan {count} models: {}", errors.join("; "))),
    }
}
