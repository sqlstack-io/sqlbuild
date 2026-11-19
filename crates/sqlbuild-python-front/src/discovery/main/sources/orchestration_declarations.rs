//! Every job, job group, calendar and sensor of `project_dir`, or `OrchestrationDeclarationError` (D017).

use crate::discovery::_helpers::failures::raised::failure_error;
use crate::discovery::_helpers::project::project_config::{configured, project_pool_limits};
use crate::discovery::_helpers::sources::orchestration_files::orchestration_error;
use crate::discovery::_helpers::sources::orchestration_values::check_orchestration_values;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_config::project::main::load_dynamic_config::load_dynamic_config;
use sqlbuild_discovery::orchestration_files::main::check_pools::check_pools;
use sqlbuild_discovery::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_discovery::tree::models::ProjectTree;
use std::path::PathBuf;

/// Every job, job group, calendar and sensor of `project_dir`, or `OrchestrationDeclarationError` (D017).
pub fn orchestration_declarations(
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<OrchestrationInventory> {
    let py: Python<'_> = project_dir.py();
    let root: PathBuf = project_dir.extract()?;
    let tree = ProjectTree::new(&root);
    let version = py.version_info();
    let inventory = py.detach(|| orchestration_inventory(&tree, (version.major, version.minor)));
    let inventory =
        inventory.map_err(|failure| orchestration_error(&tree, project_dir, failure))?;
    let limits = project_pool_limits(py, project_dir)?;
    check_pools(&inventory, &limits)
        .map_err(|failure| orchestration_error(&tree, project_dir, failure))?;
    let allow_ignore_grace: bool = configured(project_dir, load_dynamic_config)?.allow_ignore_grace;
    check_orchestration_values(py, (&inventory, allow_ignore_grace))
        .map_err(|failure| failure_error(py, failure))?;
    Ok(inventory)
}
