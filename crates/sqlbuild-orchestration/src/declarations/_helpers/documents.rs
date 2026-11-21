//! Decode the same immutable declaration documents at publication and execution boundaries, and check what a location publishes.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::declarations::models::{Definitions, GroupDefinition, JobDefinition};
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

fn document<T: DeserializeOwned>(
    documents: &BTreeMap<String, Value>,
    name: &str,
) -> Result<BTreeMap<String, T>, ControlError> {
    documents.get(name).map_or_else(
        || Ok(BTreeMap::new()),
        |value| {
            serde_json::from_value(value.clone())
                .map_err(|error| ControlError::Invalid(format!("{name}: {error}")))
        },
    )
}

pub(crate) fn decode_definitions(
    documents: &BTreeMap<String, Value>,
) -> Result<Definitions, ControlError> {
    let mut definitions = Definitions {
        jobs: document(documents, "jobs")?,
        groups: document(documents, "groups")?,
        roles: documents.get("roles").cloned(),
        ..Definitions::default()
    };
    for (name, value) in document::<Value>(documents, "triggers")? {
        if value.get("function").is_some() {
            definitions.sensors.insert(
                name,
                serde_json::from_value(value)
                    .map_err(|error| ControlError::Invalid(format!("triggers: {error}")))?,
            );
        } else {
            definitions.triggers.insert(
                name,
                serde_json::from_value(value)
                    .map_err(|error| ControlError::Invalid(format!("triggers: {error}")))?,
            );
        }
    }
    Ok(definitions)
}

/// Every declaration limits each of its groups alike, and every wait reason names known nodes.
pub(crate) fn limits_and_reasons(
    definitions: &Definitions,
    input: &SnapshotInput,
) -> Result<(), ControlError> {
    let mut limits: BTreeMap<String, NonZeroU32> = BTreeMap::new();
    for (name, job) in &definitions.jobs {
        limits = same_limits(name, job, limits)?;
    }
    for (name, group) in &definitions.groups {
        limits = same_limits(name, &group.job, limits)?;
        if !known_reasons(group, input) {
            return Err(ControlError::Invalid(format!(
                "group {name} has wait reasons outside its members' waits or the snapshot"
            )));
        }
    }
    Ok(())
}

/// `limits` with the declaration's own: no amount above its pool's limit, and one limit per pool.
fn same_limits(
    name: &str,
    job: &JobDefinition,
    mut limits: BTreeMap<String, NonZeroU32>,
) -> Result<BTreeMap<String, NonZeroU32>, ControlError> {
    for (pool, slots) in &job.pools {
        if slots.amount > slots.limit {
            return Err(ControlError::Invalid(format!(
                "{name} takes more slots of pool {pool} than its limit"
            )));
        }
        if *limits.entry(pool.clone()).or_insert(slots.limit) != slots.limit {
            return Err(ControlError::Invalid(format!(
                "pool {pool} has more than one limit"
            )));
        }
    }
    Ok(limits)
}

/// Wait reasons are shown, never scheduled by: each names a wait of its member and two snapshot nodes.
fn known_reasons(group: &GroupDefinition, input: &SnapshotInput) -> bool {
    for (member, reasons) in &group.wait_reasons {
        let Some(waits) = group.jobs.get(member) else {
            return false;
        };
        for reason in reasons {
            if !waits.contains(&reason.upstream_job)
                || !input.nodes.contains_key(&reason.from_node)
                || !input.nodes.contains_key(&reason.to_node)
            {
                return false;
            }
        }
    }
    true
}
