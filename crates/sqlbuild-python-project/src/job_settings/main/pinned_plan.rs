//! A location's plan request applies the jobs the server pinned with it.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use serde_json::Value;
use sqlbuild_orchestration::runs::models::PinnedJob;

use crate::job_settings::_helpers::pinned_build::pinned_build;
use crate::job_settings::constants::PLAN_COMMAND;
use crate::job_settings::models::PinnedBuild;

/// The jobs a plan request pins, in its project directory and target, with their build-wide `sqb plan` flags.
pub fn pinned_plan(py: Python<'_>, request: &Value) -> PyResult<PinnedBuild> {
    let invalid = |error: serde_json::Error| PyValueError::new_err(error.to_string());
    let jobs: BTreeMap<String, PinnedJob> =
        serde_json::from_value(request["jobs"].clone()).map_err(invalid)?;
    let given: BTreeSet<String> =
        serde_json::from_value(request["given"].clone()).map_err(invalid)?;
    let directory = (
        serde_json::from_value(request["project_dir"].clone()).map_err(invalid)?,
        serde_json::from_value(request["target"].clone()).map_err(invalid)?,
    );
    pinned_build(
        py,
        (PLAN_COMMAND, directory),
        (&jobs, given),
        &BTreeMap::new(),
    )
}
