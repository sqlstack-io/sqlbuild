//! Display projections of a declaration: run order of a group's members and settings safe to show.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde_json::Value;
use sqlbuild_core::redaction::main::redact_text::redact_text;
use uuid::Uuid;

use crate::declarations::constants::{
    CONCURRENCY_SETTING, DEFER_SETTING_PREFIX, IGNORE_AUDIT_FAILURES_SETTING, VARS_SETTING,
    WITHHELD_SETTING,
};
use crate::declarations::models::{
    Definitions, GroupDefinition, JobDefinition, JobMember, PoolSlots, PoolUse, ServerPoolCaps,
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

/// The pools a declaration takes, keyed by name.
pub(crate) type DeclaredPools<'a> = (&'a str, &'a BTreeMap<String, PoolSlots>);

/// The caps of this server's launcher for the job's location and target, by pool.
pub(crate) type Caps = BTreeMap<String, NonZeroU32>;

/// Each pool `job` takes, with its slots, this server's limit, and the other declarations in `all` taking it.
pub(crate) fn pool_uses(
    (job_key, job): (&str, &JobDefinition),
    all: &[DeclaredPools<'_>],
    caps: &Caps,
) -> Vec<PoolUse> {
    let mut uses: Vec<PoolUse> = Vec::new();
    for (pool, slots) in &job.pools {
        let cap = caps.get(pool).map(|cap| cap.get());
        let server_limit = cap.map_or(slots.limit.get(), |cap| cap.min(slots.limit.get()));
        uses.push(PoolUse {
            pool: pool.clone(),
            amount: slots.amount.get(),
            limit: slots.limit.get(),
            cap,
            server_limit,
            clamped: slots.amount.get() > server_limit,
            shared_with: sharing(pool, job_key, all),
        });
    }
    uses
}

/// The declarations in `all` other than `job_key` that take `pool`.
fn sharing(pool: &str, job_key: &str, all: &[DeclaredPools<'_>]) -> BTreeSet<String> {
    let mut shared: BTreeSet<String> = BTreeSet::new();
    for (other, pools) in all {
        if *other != job_key && pools.contains_key(pool) {
            shared.insert((*other).to_owned());
        }
    }
    shared
}

/// The job's settings as people see them, and its own `ignore_audit_failures` setting.
pub(crate) fn declared_settings(job: &JobDefinition) -> (BTreeMap<String, Value>, Option<bool>) {
    (shown_settings(&job.settings), ignores_audit_failures(job))
}

/// The job's own `ignore_audit_failures` setting, when it sets one.
pub(crate) fn ignores_audit_failures(job: &JobDefinition) -> Option<bool> {
    job.settings
        .get(IGNORE_AUDIT_FAILURES_SETTING)
        .and_then(Value::as_bool)
}

/// The job's own `concurrency` setting, when it is a whole number.
pub(crate) fn run_concurrency(job: &JobDefinition) -> Option<u64> {
    job.settings
        .get(CONCURRENCY_SETTING)
        .and_then(Value::as_u64)
}

/// The groups running `job_key`, its pools among every job and group of its snapshot, and its run concurrency.
pub(crate) fn snapshot_relations(
    definitions: &Definitions,
    (job_key, job): (&str, &JobDefinition),
    (caps, location): (&ServerPoolCaps, (Uuid, &str)),
) -> (BTreeSet<String>, Vec<PoolUse>, Option<u64>) {
    let mut all: Vec<DeclaredPools<'_>> = Vec::new();
    for (name, other) in &definitions.jobs {
        all.push((name, &other.pools));
    }
    for (name, group) in &definitions.groups {
        all.push((name, &group.job.pools));
    }
    (
        groups_running(definitions, job_key),
        pool_uses((job_key, job), &all, &caps.on(location)),
        run_concurrency(job),
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
