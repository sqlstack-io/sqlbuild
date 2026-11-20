//! Every job and group a project declares, for resolving `job(...)` dependencies.

use crate::job_settings::_helpers::job_catalog::catalog_where;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::graph::models::JobSelection;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use std::collections::BTreeMap;

/// Every job and group the project declares, whatever targets they are declared for.
pub fn declared_jobs(project_dir: &Bound<'_, PyAny>) -> PyResult<BTreeMap<String, JobSelection>> {
    let inventory: OrchestrationInventory = orchestration_declarations(project_dir)?;
    Ok(catalog_where(&inventory, |_| true).0)
}
