//! The selections a project's jobs and sensors make, gathered from its orchestration inventory.

use sqlbuild_discovery::orchestration_files::models::{
    FilePosition, NamedModule, OrchestrationInventory,
};

use crate::orchestration_rules::constants::{JOB_OWNER, SENSOR_OWNER};
use crate::orchestration_rules::models::{OrchestrationFacts, OrchestrationSelection};

/// The selections in `inventory`, and the job and job group names a selector may use.
pub(crate) fn facts(inventory: OrchestrationInventory) -> OrchestrationFacts {
    let mut selections: Vec<OrchestrationSelection> = inventory
        .jobs
        .iter()
        .map(|(path, job)| {
            selection(
                (path, JOB_OWNER, &job.run.name.value),
                Some(job.select.value.clone()),
                job.select.position,
            )
        })
        .collect();
    selections.extend(inventory.sensors.iter().flat_map(sensor_selections));
    let run_names: Vec<String> = inventory
        .jobs
        .iter()
        .map(|(_, job)| job.run.name.value.clone())
        .chain(
            inventory
                .groups
                .iter()
                .map(|(_, group)| group.run.name.value.clone()),
        )
        .collect();
    OrchestrationFacts {
        selections,
        run_names,
    }
}

/// One selection per `select` the sensor's conditions write.
fn sensor_selections(sensor: &NamedModule) -> Vec<OrchestrationSelection> {
    sensor
        .selections
        .iter()
        .map(|condition| {
            selection(
                (&sensor.path, SENSOR_OWNER, &sensor.name),
                condition.value.clone(),
                condition.position,
            )
        })
        .collect()
}

fn selection(
    (path, owner, name): (&str, &str, &str),
    text: Option<String>,
    position: FilePosition,
) -> OrchestrationSelection {
    OrchestrationSelection {
        path: path.to_owned(),
        owner: owner.to_owned(),
        name: name.to_owned(),
        selection: text,
        line: position.line as u64,
        column: position.column as u64,
    }
}
