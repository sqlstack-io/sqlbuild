//! Refuse a job or job group naming a concurrency group the project does not declare.

use std::collections::BTreeMap;

use crate::orchestration_files::_helpers::inventory_files::failure;
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{OrchestrationFailure, OrchestrationInventory, RunFile};

/// The first job or job group naming a group missing from `declared`, at its name.
pub fn check_concurrency_groups(
    inventory: &OrchestrationInventory,
    declared: &BTreeMap<String, u32>,
) -> Result<(), OrchestrationFailure> {
    let runs = inventory
        .jobs
        .iter()
        .map(|(path, job)| (path, ("job", &job.run)))
        .chain(
            inventory
                .groups
                .iter()
                .map(|(path, group)| (path, ("job group", &group.run))),
        );
    for (path, (kind, run)) in runs {
        if let Some(group) = undeclared(run, declared) {
            return Err(failure(
                path,
                FileError {
                    message: format!(
                        "{kind} '{}' names concurrency group '{group}', which [concurrency_groups] \
                         in sqlbuild_project.toml does not declare",
                        run.name.value
                    ),
                    position: run.name.position,
                    help: Some(declare_help(group, declared)),
                },
            ));
        }
    }
    Ok(())
}

fn undeclared<'a>(run: &'a RunFile, declared: &BTreeMap<String, u32>) -> Option<&'a str> {
    run.concurrency_groups
        .iter()
        .find(|group| !declared.contains_key(*group))
        .map(String::as_str)
}

fn declare_help(group: &str, declared: &BTreeMap<String, u32>) -> String {
    let declare = format!("declare it with its most runs at once, such as {group} = 1");
    if declared.is_empty() {
        return format!("the project declares no concurrency groups; {declare}");
    }
    let names: Vec<&str> = declared.keys().map(String::as_str).collect();
    format!("declared groups: {}; or {declare}", names.join(", "))
}
