//! Validate resolved declarations before acquiring the snapshot publication lock.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;

use crate::declarations::models::{Definitions, Schedule};
use crate::scheduler::constants::MAX_TICKS_PER_BATCH;
use crate::scheduler::main::parse_cron::parse_cron;
use crate::scheduler::models::CatchUp;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

fn document<T: DeserializeOwned>(
    input: &SnapshotInput,
    name: &str,
) -> Result<BTreeMap<String, T>, ControlError> {
    input.documents.get(name).map_or_else(
        || Ok(BTreeMap::new()),
        |value| {
            serde_json::from_value(value.clone())
                .map_err(|error| ControlError::Invalid(format!("{name}: {error}")))
        },
    )
}

/// Names, references and resolved selections are checked again at the native trust boundary.
pub fn snapshot_definitions(input: &SnapshotInput) -> Result<Definitions, ControlError> {
    let definitions = Definitions {
        jobs: document(input, "jobs")?,
        triggers: document(input, "triggers")?,
    };
    for (name, job) in &definitions.jobs {
        if name.trim().is_empty()
            || job.description.trim().is_empty()
            || job.selection.is_empty()
            || job
                .selection
                .iter()
                .any(|key| !input.nodes.contains_key(key))
        {
            return Err(ControlError::Invalid(format!(
                "job {name} needs a description and a nonempty selection of snapshot nodes"
            )));
        }
    }
    for (name, trigger) in &definitions.triggers {
        if name.trim().is_empty()
            || trigger.description.trim().is_empty()
            || !definitions.jobs.contains_key(&trigger.job_key)
        {
            return Err(ControlError::Invalid(format!(
                "trigger {name} needs a description and a known job"
            )));
        }
        if let CatchUp::All { max_runs } = trigger.catch_up
            && max_runs.get() > MAX_TICKS_PER_BATCH
        {
            return Err(ControlError::Invalid(format!(
                "trigger {name} catch-up exceeds {MAX_TICKS_PER_BATCH}"
            )));
        }
        match &trigger.schedule {
            Schedule::Cron {
                expression,
                timezone,
            } => {
                parse_cron(expression, timezone)?;
            }
            Schedule::Calendar { times, .. } if times.len() > MAX_TICKS_PER_BATCH => {
                return Err(ControlError::Invalid(format!(
                    "calendar {name} exceeds {MAX_TICKS_PER_BATCH} upcoming times"
                )));
            }
            Schedule::Calendar { .. } => {}
        }
    }
    Ok(definitions)
}
