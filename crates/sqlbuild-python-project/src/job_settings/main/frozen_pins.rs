//! The jobs a frozen ad-hoc build pins, as its worker applies them.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::Value;
use sqlbuild_orchestration::runs::models::{RunCommand, RunRequest};

use crate::job_settings::_helpers::pinned_build::pinned_build;
use crate::job_settings::models::PinnedBuild;

/// A build's pinned jobs, less `skipped` nodes and the `settings` its command line gives; `None` for a job's run or another verb.
pub fn frozen_pins(
    project_dir: &Bound<'_, PyAny>,
    frozen: &RunRequest,
    (settings, skipped): (&BTreeMap<String, Value>, &BTreeMap<String, String>),
) -> PyResult<Option<PinnedBuild>> {
    if frozen.command != RunCommand::Build || frozen.job_key.is_some() {
        return Ok(None);
    }
    let directory = (project_dir.extract()?, Some(frozen.target.clone()));
    let given: BTreeSet<String> = settings.keys().cloned().collect();
    let command = frozen.command.as_str();
    pinned_build(
        project_dir.py(),
        (command, directory),
        (&frozen.jobs, given),
        skipped,
    )
    .map(Some)
}
