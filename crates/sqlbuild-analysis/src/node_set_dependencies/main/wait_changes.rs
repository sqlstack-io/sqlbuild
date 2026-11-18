//! `wait_changes`: what a Python node newly waits on, or no longer, through its set dependencies.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::node_set_dependencies::constants::{ENDS_KEY, SET_DEPENDENCIES_KEY, SET_KEY};

/// Each end node `node` gained or lost per set since its last build; nothing if that recorded none.
#[must_use]
pub fn wait_changes(node: &str, previous: Option<&str>, current: Option<&str>) -> Vec<String> {
    let (Some(previous), Some(current)) = (previous.and_then(sets), current.and_then(sets)) else {
        return Vec::new();
    };
    let labels: BTreeSet<&String> = previous.keys().chain(current.keys()).collect();
    let empty: BTreeSet<String> = BTreeSet::new();
    let mut lines: Vec<String> = Vec::new();
    for label in labels {
        let before: &BTreeSet<String> = previous.get(label).unwrap_or(&empty);
        let after: &BTreeSet<String> = current.get(label).unwrap_or(&empty);
        lines.extend(
            after
                .difference(before)
                .map(|end| format!("{node} now also waits on {end} through {label}")),
        );
        lines.extend(
            before
                .difference(after)
                .map(|end| format!("{node} no longer waits on {end} through {label}")),
        );
    }
    lines
}

/// Each recorded set's end node labels, or `None` when the metadata records no sets.
fn sets(metadata_json: &str) -> Option<BTreeMap<String, BTreeSet<String>>> {
    let Ok(metadata) = serde_json::from_str::<Value>(metadata_json) else {
        return None;
    };
    let recorded: &Vec<Value> = metadata.get(SET_DEPENDENCIES_KEY)?.as_array()?;
    Some(recorded.iter().filter_map(recorded_set).collect())
}

/// One recorded set's label and end node labels.
fn recorded_set(set: &Value) -> Option<(String, BTreeSet<String>)> {
    let label: String = set.get(SET_KEY)?.as_str()?.to_owned();
    let ends: BTreeSet<String> = set
        .get(ENDS_KEY)?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Some((label, ends))
}
