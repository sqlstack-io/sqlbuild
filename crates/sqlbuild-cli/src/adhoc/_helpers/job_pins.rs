//! The jobs an ad-hoc build names: the build-wide settings they give it and the frozen nodes each pins.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::job_settings::errors::NodeJobSettingsError;
use sqlbuild_analysis::job_settings::main::pinned_job_nodes::pinned_job_nodes;
use sqlbuild_analysis::job_settings::main::resolve_job_settings::resolve_job_settings;
use sqlbuild_analysis::job_settings::models::DeclaredRun;
use sqlbuild_orchestration::runs::models::PinnedJob;

use crate::adhoc::_helpers::planner::{key_text, selected_keys};
use crate::adhoc::_helpers::publication::IndexedPublication;
use crate::adhoc::models::Refusal;

/// Each published job with its settings and each group with its jobs, as a build declares them.
fn declared(publication: &IndexedPublication) -> BTreeMap<String, DeclaredRun> {
    let jobs = &publication.jobs;
    jobs.selections
        .keys()
        .map(|name| {
            let run = match jobs.groups.get(name) {
                Some(members) => DeclaredRun::Group {
                    members: members.clone(),
                },
                None => DeclaredRun::Job {
                    settings: jobs.settings.get(name).cloned().unwrap_or_default(),
                    retry: None,
                },
            };
            (name.clone(), run)
        })
        .collect()
}

fn refusal(error: NodeJobSettingsError) -> Refusal {
    match error {
        NodeJobSettingsError::Conflict(conflict) => Refusal::Selection(conflict.message),
        NodeJobSettingsError::Selector(failure) => Refusal::Selector(failure.error),
    }
}

/// The jobs `select` names and the build-wide settings they give where the command's `settings` are silent.
pub(crate) fn named_job_settings(
    publication: &IndexedPublication,
    select: &[String],
    settings: &BTreeMap<String, Json>,
) -> Result<(Vec<String>, BTreeMap<String, Json>), Refusal> {
    let given: BTreeSet<String> = settings.keys().cloned().collect();
    resolve_job_settings(declared(publication), (select, &given), None)
        .map(|resolved| (resolved.named, resolved.build))
        .map_err(refusal)
}

/// Each job `select` names, pinned to the nodes of `selection` it gives its settings to, as `sqb build` decides.
pub(crate) fn pinned_jobs(
    publication: &IndexedPublication,
    (select, exclude): (&[String], &[String]),
    include_python: bool,
    selection: &BTreeSet<String>,
) -> Result<BTreeMap<String, PinnedJob>, Refusal> {
    let token_nodes =
        |token: &str, exclude: &[String]| -> Result<BTreeSet<ObjectKey>, SelectorError> {
            selected_keys(publication, &[token.to_owned()], exclude, include_python)
        };
    let pinned = pinned_job_nodes(
        publication.graph_with_jobs(include_python),
        (select, exclude),
        &declared(publication),
        &token_nodes,
    )
    .map_err(|failure| Refusal::Selector(failure.error))?;
    Ok(pinned
        .into_iter()
        .map(|(name, keys)| {
            let job = PinnedJob {
                selection: frozen_keys(publication, &keys, selection),
                settings: publication
                    .jobs
                    .settings
                    .get(&name)
                    .cloned()
                    .unwrap_or_default(),
            };
            (name, job)
        })
        .collect())
}

/// A job's selected graph keys as the frozen node keys of `selection`.
fn frozen_keys(
    publication: &IndexedPublication,
    keys: &BTreeSet<ObjectKey>,
    selection: &BTreeSet<String>,
) -> BTreeSet<String> {
    keys.iter()
        .map(|key| key_text(publication, key))
        .filter(|key| selection.contains(key))
        .collect()
}
