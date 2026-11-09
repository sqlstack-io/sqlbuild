//! Display projections of a declaration: run order of a group's members and settings safe to show.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sqlbuild_core::redaction::main::redact_text::redact_text;

use crate::declarations::constants::{VARS_SETTING, WITHHELD_SETTING};
use crate::declarations::models::{Definitions, GroupDefinition, JobMember};
use crate::snapshots::models::SnapshotNode;

/// Members whose waits are all earlier come next, by name among equals; validation rejected cycles.
pub(crate) fn members_in_order(group: &GroupDefinition) -> Vec<JobMember> {
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    let mut members: Vec<JobMember> = Vec::new();
    while members.len() < group.jobs.len() {
        let ready = ready_members(group, &placed);
        if ready.is_empty() {
            break;
        }
        for (name, waits) in ready {
            placed.insert(name);
            members.push(JobMember {
                job_key: name.clone(),
                waits_for: waits.clone(),
            });
        }
    }
    members
}

/// Unplaced members whose every wait is already placed.
fn ready_members<'a>(
    group: &'a GroupDefinition,
    placed: &BTreeSet<&str>,
) -> Vec<(&'a String, &'a BTreeSet<String>)> {
    group
        .jobs
        .iter()
        .filter(|(name, waits)| !placed.contains(name.as_str()) && all_placed(waits, placed))
        .collect()
}

fn all_placed(waits: &BTreeSet<String>, placed: &BTreeSet<&str>) -> bool {
    waits.iter().all(|wait| placed.contains(wait.as_str()))
}

/// `vars` is free-form and can carry credentials, so only its presence shows.
pub(crate) fn shown_settings(settings: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    settings
        .iter()
        .map(|(name, value)| {
            let shown = match value {
                _ if name == VARS_SETTING => Value::String(WITHHELD_SETTING.into()),
                Value::String(text) => Value::String(redact_text(text)),
                other => other.clone(),
            };
            (name.clone(), shown)
        })
        .collect()
}

/// The groups of the snapshot that run `job_key`.
pub(crate) fn groups_running(definitions: &Definitions, job_key: &str) -> BTreeSet<String> {
    definitions
        .groups
        .iter()
        .filter(|(_, group)| group.jobs.contains_key(job_key))
        .map(|(name, _)| name.clone())
        .collect()
}

/// Selected nodes marked critical, whose failure stops a group.
pub(crate) fn critical_nodes(nodes: &BTreeMap<String, SnapshotNode>) -> BTreeSet<String> {
    nodes
        .iter()
        .filter(|(_, node)| node.definition["config"]["critical"] == Value::Bool(true))
        .map(|(key, _)| key.clone())
        .collect()
}
