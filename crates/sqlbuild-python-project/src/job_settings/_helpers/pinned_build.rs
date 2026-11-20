//! A frozen ad-hoc build or plan applies the jobs it pins as a command naming them would.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_cli::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use sqlbuild_orchestration::runs::models::PinnedJob;

use crate::job_settings::_helpers::job_catalog::with_frozen_jobs;
use crate::job_settings::_helpers::named_job_build_settings::scoped_job_build_settings;
use crate::job_settings::models::PinnedBuild;
use crate::job_settings::types::PinnedJobs;

/// The pinned jobs less `skipped` nodes, their `job:` tokens and `command`'s flags for their build-wide settings, less those `given`.
pub(crate) fn pinned_build(
    py: Python<'_>,
    (command, directory): (&str, (PathBuf, Option<String>)),
    (jobs, given): (&BTreeMap<String, PinnedJob>, BTreeSet<String>),
    skipped: &BTreeMap<String, String>,
) -> PyResult<PinnedBuild> {
    let pinned: PinnedJobs = jobs
        .iter()
        .map(|(name, job)| {
            (
                name.clone(),
                (frozen_keys(job, skipped), job.settings.clone()),
            )
        })
        .collect();
    let tokens: Vec<String> = jobs.keys().map(|name| format!("job:{name}")).collect();
    let mut flags: Vec<String> = Vec::new();
    if !jobs.is_empty() {
        let settings = with_frozen_jobs(directory.clone(), pinned.clone(), || {
            scoped_job_build_settings(py, (&tokens, &given), &directory.0)
        })?;
        flags = accepted_settings_arguments(command, &settings).map_err(PyValueError::new_err)?;
    }
    Ok(PinnedBuild {
        directory,
        jobs: pinned,
        tokens,
        flags,
        given,
    })
}

/// A pinned job's frozen node keys that still run.
fn frozen_keys(job: &PinnedJob, skipped: &BTreeMap<String, String>) -> BTreeSet<ObjectKey> {
    let mut keys: BTreeSet<ObjectKey> = BTreeSet::new();
    for key in job
        .selection
        .iter()
        .filter(|key| !skipped.contains_key(*key))
    {
        if let Some((kind, name)) = key.split_once(':') {
            keys.insert((kind.to_owned(), name.to_owned()));
        }
    }
    keys
}
