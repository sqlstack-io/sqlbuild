//! The jobs an ad-hoc build pins: which apply settings, what those settings are, and how a narrowed run keeps them.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::runs::models::{PinnedJob, RunRequest};

/// Keep each pinned job's nodes that `request` still selects; a job left with none stays named.
pub(crate) fn narrow_pinned_jobs(request: &mut RunRequest) {
    for job in request.jobs.values_mut() {
        job.selection.retain(|key| request.selection.contains(key));
    }
}

/// The build-wide settings a run applies: the command line's, then those its pinned jobs give.
pub(crate) fn build_settings(
    (settings, job_settings): (&BTreeMap<String, Value>, &BTreeMap<String, Value>),
) -> BTreeMap<String, Value> {
    let mut effective: BTreeMap<String, Value> = job_settings.clone();
    effective.extend(
        settings
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    effective
}

/// The pinned jobs that give `request` a setting: to some of its nodes, or to the whole build.
pub(crate) fn applying_jobs(request: &RunRequest) -> Vec<(&String, &PinnedJob)> {
    request
        .jobs
        .iter()
        .filter(|(_, job)| !job.selection.is_empty() || gives_build_settings(request, job))
        .collect()
}

/// Whether `job` set one of the build-wide settings `request` takes from its jobs.
pub(crate) fn gives_build_settings(request: &RunRequest, job: &PinnedJob) -> bool {
    request
        .job_settings
        .iter()
        .any(|(name, value)| job.settings.get(name) == Some(value))
}

/// Every setting `request` applies anywhere: build-wide, then each applying job's own for its nodes.
pub(crate) fn applied_settings(request: &RunRequest) -> BTreeMap<String, Value> {
    let mut applied: BTreeMap<String, Value> =
        build_settings((&request.settings, &request.job_settings));
    for (_, job) in applying_jobs(request) {
        for (name, value) in &job.settings {
            if !request.settings.contains_key(name) && !applied.contains_key(name) {
                applied.insert(name.clone(), value.clone());
            }
        }
    }
    applied
}
