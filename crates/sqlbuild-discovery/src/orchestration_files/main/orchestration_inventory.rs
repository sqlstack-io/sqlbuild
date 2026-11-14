//! Discover, read and check every orchestration declaration without importing project Python.

use crate::constants::ORCHESTRATION_DIRECTORY;
use crate::orchestration_files::_helpers::inventory_files::{
    check_top_level, failure, read_groups, read_jobs, read_python_modules,
};
use crate::orchestration_files::_helpers::references::check_references;
use crate::orchestration_files::models::{
    GroupFile, JobFile, OrchestrationFailure, OrchestrationInventory, PythonModules,
};
use crate::tree::models::ProjectTree;

/// Every job, group, calendar and sensor below `orchestration/`, or the first problem with them.
pub fn orchestration_inventory(
    tree: &ProjectTree,
    python_version: (u8, u8),
) -> Result<OrchestrationInventory, OrchestrationFailure> {
    if !tree.is_dir(ORCHESTRATION_DIRECTORY) {
        return Ok(OrchestrationInventory::default());
    }
    check_top_level(tree)?;
    let jobs: Vec<(String, JobFile)> = read_jobs(tree)?;
    let groups: Vec<(String, GroupFile)> = read_groups(tree)?;
    let PythonModules {
        calendars,
        sensors,
        modules,
    } = read_python_modules(tree, python_version)?;
    let inventory: OrchestrationInventory = OrchestrationInventory {
        jobs,
        groups,
        calendars,
        sensors,
        modules,
    };
    check_references(&inventory).map_err(|(path, error)| failure(&path, error))?;
    Ok(inventory)
}
