//! The selections a project's jobs and sensors make, gathered from its orchestration inventory.

use sqlbuild_discovery::orchestration_files::models::{
    FilePosition, NamedModule, OrchestrationInventory,
};

use sqlbuild_pysource::orchestration_modules::models::{ConditionSource, SourcePosition};

use crate::orchestration_rules::_helpers::imports::{ImportReader, resolved_condition};
use crate::orchestration_rules::constants::{JOB_OWNER, SENSOR_OWNER};
use crate::orchestration_rules::models::{OrchestrationFacts, OrchestrationSelection};

/// The selections in `inventory`, and the job and job group names a selector may use.
pub(crate) fn facts(
    inventory: OrchestrationInventory,
    reader: &ImportReader<'_>,
) -> OrchestrationFacts {
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
    selections.extend(
        inventory
            .sensors
            .iter()
            .filter_map(|sensor| sensor_selection(sensor, reader)),
    );
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

/// The selection the sensor's condition makes, following imports of project names; none when it selects no nodes.
fn sensor_selection(
    sensor: &NamedModule,
    reader: &ImportReader<'_>,
) -> Option<OrchestrationSelection> {
    let owner = (sensor.path.as_str(), SENSOR_OWNER, sensor.name.as_str());
    Some(
        match resolved_condition(reader, &sensor.path, sensor.condition.clone())? {
            ConditionSource::Selection(select) => {
                selection(owner, select.value, file_position(select.position))
            }
            ConditionSource::Unresolvable { what, position } => OrchestrationSelection {
                unresolved: Some(what),
                ..selection(owner, None, file_position(position))
            },
            ConditionSource::Imported { position, .. } => {
                selection(owner, None, file_position(position))
            }
        },
    )
}

fn file_position(position: SourcePosition) -> FilePosition {
    FilePosition {
        line: position.line,
        column: position.column,
    }
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
        unresolved: None,
        line: position.line as u64,
        column: position.column as u64,
    }
}
