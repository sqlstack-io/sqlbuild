//! Display projections of a declaration: run order of a group's members and settings safe to show.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sqlbuild_core::redaction::main::redact_text::redact_text;

use crate::declarations::constants::{DEFER_SETTING_PREFIX, VARS_SETTING, WITHHELD_SETTING};
use crate::declarations::models::{
    ConcurrencyRule, Definitions, GroupDefinition, JobDefinition, JobMember,
};
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
                reasons: group.wait_reasons.get(name).cloned().unwrap_or_default(),
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

/// Each of `job`'s concurrency groups with its limit and the other declarations in `all` naming it.
pub(crate) fn concurrency_rules(
    (job_key, job): (&str, &JobDefinition),
    all: &[(&str, &BTreeSet<String>)],
) -> Vec<ConcurrencyRule> {
    let mut rules: Vec<ConcurrencyRule> = Vec::new();
    for group in &job.concurrency_groups {
        rules.push(ConcurrencyRule {
            group: group.clone(),
            limit: job.concurrency_limits.get(group).map(|limit| limit.get()),
            shared_with: sharing(group, job_key, all),
        });
    }
    rules
}

/// The declarations in `all` other than `job_key` that name `group`.
fn sharing(group: &str, job_key: &str, all: &[(&str, &BTreeSet<String>)]) -> BTreeSet<String> {
    let mut shared: BTreeSet<String> = BTreeSet::new();
    for (other, groups) in all {
        if *other != job_key && groups.contains(group) {
            shared.insert((*other).to_owned());
        }
    }
    shared
}

/// The groups running `job_key`, and its concurrency rules among every job and group of its snapshot.
pub(crate) fn snapshot_relations(
    definitions: &Definitions,
    (job_key, job): (&str, &JobDefinition),
) -> (BTreeSet<String>, Vec<ConcurrencyRule>) {
    let mut all: Vec<(&str, &BTreeSet<String>)> = Vec::new();
    for (name, other) in &definitions.jobs {
        all.push((name, &other.concurrency_groups));
    }
    for (name, group) in &definitions.groups {
        all.push((name, &group.job.concurrency_groups));
    }
    (
        groups_running(definitions, job_key),
        concurrency_rules((job_key, job), &all),
    )
}

/// `vars` can carry credentials and a deferral can name a storage path, so only their presence shows.
pub(crate) fn shown_settings(settings: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    settings
        .iter()
        .map(|(name, value)| {
            let shown = match value {
                _ if withheld(name, value) => Value::String(WITHHELD_SETTING.into()),
                Value::String(text) => Value::String(redact_text(text)),
                other => other.clone(),
            };
            (name.clone(), shown)
        })
        .collect()
}

fn withheld(name: &str, value: &Value) -> bool {
    name == VARS_SETTING
        || (name.starts_with(DEFER_SETTING_PREFIX) && !value.as_str().is_some_and(target_name))
}

/// Target names are TOML bare keys in the project file: letters, digits, `_` and `-`.
fn target_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The groups of the snapshot that run `job_key`.
fn groups_running(definitions: &Definitions, job_key: &str) -> BTreeSet<String> {
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

/// The one cursor type of the selected nodes that have a cursor, so a backfill window can be typed.
pub(crate) fn cursor_type(nodes: &BTreeMap<String, SnapshotNode>) -> Option<String> {
    let types: BTreeSet<&str> = nodes
        .values()
        .filter_map(|node| node.definition["config"]["cursor_type"].as_str())
        .collect();
    let mut types = types.into_iter();
    match (types.next(), types.next()) {
        (Some(only), None) => Some(only.to_owned()),
        _ => None,
    }
}
