//! The project's orchestration selections, read from disk only when an orchestration rule is selected.

use std::path::Path;

use sqlbuild_discovery::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use sqlbuild_discovery::orchestration_files::models::{FilePosition, OrchestrationInventory};
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::models::EvaluateRequest;
use crate::orchestration_rules::constants::{JOB_OWNER, NAMED_SELECTOR_LIMIT_CODE, SENSOR_OWNER};
use crate::orchestration_rules::models::{OrchestrationFacts, OrchestrationSelection};
use crate::rules::main::request_selects::request_selects;

/// Every job and sensor-condition selection when the request selects an orchestration rule, otherwise none.
pub(crate) fn orchestration_facts(request: &EvaluateRequest) -> Result<OrchestrationFacts, String> {
    if !request_selects(request, |rule| rule.code == NAMED_SELECTOR_LIMIT_CODE)? {
        return Ok(OrchestrationFacts::default());
    }
    let tree = ProjectTree::new(Path::new(&request.project_dir));
    let inventory: OrchestrationInventory = orchestration_inventory(&tree, request.python_version)
        .map_err(|failure| format!("cannot read the orchestration declarations: {failure:?}"))?;
    Ok(facts(inventory))
}

fn facts(inventory: OrchestrationInventory) -> OrchestrationFacts {
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
    selections.extend(inventory.sensors.iter().flat_map(|sensor| {
        sensor.selections.iter().map(|condition| {
            selection(
                (&sensor.path, SENSOR_OWNER, &sensor.name),
                condition.value.clone(),
                condition.position,
            )
        })
    }));
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
