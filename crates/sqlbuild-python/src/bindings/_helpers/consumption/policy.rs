//! What `--changed` counts and how far a change reaches: the command line, else `[changes]`.

use std::collections::BTreeSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_state::consumption::models::{
    ChangeKind, ChangedPolicy, NodeKey, Reach, TriggeredJob,
};

use crate::bindings::_helpers::compiled_project::job_catalog::triggered_jobs;
use crate::bindings::_helpers::discovery::project_config::configured_changes;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::records::fields::field as native_field;

const DEFINITION: &str = "definition";
const SOURCE_DATA: &str = "source_data";
const REACH_JOB: &str = "job";
const REACH_ALL: &str = "all";

fn change_kind(name: &str) -> PyResult<ChangeKind> {
    match name {
        DEFINITION => Ok(ChangeKind::Definition),
        SOURCE_DATA => Ok(ChangeKind::SourceData),
        other => Err(PyValueError::new_err(format!(
            "unknown change kind '{other}': expected {DEFINITION} or {SOURCE_DATA}"
        ))),
    }
}

/// A change kind as `[changes] counts` and `--changed=` spell it.
pub(crate) const fn kind_value(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Definition => DEFINITION,
        ChangeKind::SourceData => SOURCE_DATA,
    }
}

/// A triggered job with its selected nodes as consumption keys.
fn triggered_job((name, keys): (String, BTreeSet<ObjectKey>)) -> TriggeredJob {
    TriggeredJob {
        name,
        nodes: keys
            .into_iter()
            .map(|(kind, name)| NodeKey { kind, name })
            .collect(),
    }
}

/// A reach as `[changes] reach` and `--reach` spell it.
pub(crate) const fn reach_value(reach: &Reach) -> &'static str {
    match reach {
        Reach::Job(_) => REACH_JOB,
        Reach::All => REACH_ALL,
    }
}

/// The policy of a `--changed` request, or None without it; triggered jobs resolve on `project`.
pub(crate) fn changed_policy(
    project_dir: &Bound<'_, PyAny>,
    request: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Option<ChangedPolicy>> {
    if !native_field(request, "changed")?.is_truthy()? {
        return Ok(None);
    }
    let configured = configured_changes(project_dir)?;
    let counts: Vec<String> = match native_field(request, "changed_counts")?.extract()? {
        Some(counts) => counts,
        None => configured
            .counts
            .unwrap_or_else(|| vec![DEFINITION.to_owned()]),
    };
    let reach: String = match native_field(request, "reach")?.extract()? {
        Some(reach) => reach,
        None => configured.reach.unwrap_or_else(|| REACH_JOB.to_owned()),
    };
    let counts: BTreeSet<ChangeKind> = counts
        .iter()
        .map(|name| change_kind(name))
        .collect::<PyResult<_>>()?;
    let reach: Reach = match reach.as_str() {
        REACH_ALL => Reach::All,
        REACH_JOB => {
            let graph: PyRef<'_, NativeProjectGraph> =
                native_field(project, "lineage_graph")?.extract()?;
            Reach::Job(
                triggered_jobs(project.py(), graph.graph())?
                    .into_iter()
                    .map(triggered_job)
                    .collect(),
            )
        }
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown reach '{other}': expected {REACH_JOB} or {REACH_ALL}"
            )));
        }
    };
    Ok(Some(ChangedPolicy { counts, reach }))
}

/// One line saying what the policy counts and how far it reaches, as build and plan print it.
pub(crate) fn policy_line(policy: &ChangedPolicy) -> String {
    let label = |kind: &ChangeKind| match kind {
        ChangeKind::Definition => "definition",
        ChangeKind::SourceData => "source data",
    };
    let counted: Vec<&str> = policy.counts.iter().map(label).collect();
    let uncounted: Vec<String> = [ChangeKind::Definition, ChangeKind::SourceData]
        .iter()
        .filter(|kind| !policy.counts.contains(kind))
        .map(|kind| format!(" ({} not counted)", label(kind)))
        .collect();
    format!(
        "Counting: {}{} · reach: {}",
        counted.join(", "),
        uncounted.concat(),
        reach_value(&policy.reach)
    )
}
