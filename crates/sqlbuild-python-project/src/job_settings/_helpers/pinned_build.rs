//! A frozen ad-hoc build or plan applies the jobs it pins as a command naming them would.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cli::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use sqlbuild_orchestration::runs::models::PinnedJob;

use crate::job_settings::_helpers::frozen_selector::frozen_key_selectors;
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
            Ok((
                name.clone(),
                (
                    frozen_key_selectors(&job.selection, skipped)?,
                    job.settings.clone(),
                ),
            ))
        })
        .collect::<PyResult<_>>()?;
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
