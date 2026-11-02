//! Native publication validates every sensor before any snapshot pointer moves.

use crate::declarations::models::Definitions;
use crate::sensors::constants::SOURCE_NODE_KIND;
use crate::sensors::models::SensorCondition;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

pub(crate) fn validate_sensors(
    definitions: &Definitions,
    input: &SnapshotInput,
) -> Result<(), ControlError> {
    for (name, sensor) in &definitions.sensors {
        let valid_condition = match &sensor.when {
            SensorCondition::Custom {
                function,
                code_hash,
            } => !function.trim().is_empty() && !code_hash.is_empty(),
            SensorCondition::PlatformEvent { kinds } => !kinds.is_empty(),
            SensorCondition::NodeEvent { selection, kinds } => {
                !kinds.is_empty()
                    && !selection.is_empty()
                    && selection.iter().all(|key| input.nodes.contains_key(key))
            }
            SensorCondition::SourceNewData { selection }
            | SensorCondition::SourceStale { selection, .. } => {
                !selection.is_empty()
                    && selection.iter().all(|key| {
                        input
                            .nodes
                            .get(key)
                            .is_some_and(|node| node.kind == SOURCE_NODE_KIND)
                    })
            }
            SensorCondition::RunEvent {
                job_key,
                kinds,
                executed_successfully,
            } => {
                !kinds.is_empty()
                    && definitions
                        .jobs
                        .get(job_key)
                        .or_else(|| definitions.groups.get(job_key).map(|group| &group.job))
                        .is_some_and(|source| {
                            executed_successfully.as_ref().is_none_or(|nodes| {
                                !nodes.is_empty() && nodes.is_subset(&source.selection)
                            })
                        })
            }
        };
        if name.trim().is_empty()
            || sensor.description.trim().is_empty()
            || !valid_condition
            || (!definitions.jobs.contains_key(&sensor.job_key)
                && !definitions.groups.contains_key(&sensor.job_key))
        {
            return Err(ControlError::Invalid(format!(
                "sensor {name} needs a description, a known job and nonempty event kinds; executed_successfully must select nonempty source job/group nodes; node_event must select nonempty snapshot nodes; source_new_data and source_stale must select nonempty snapshot sources"
            )));
        }
    }
    Ok(())
}
