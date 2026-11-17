//! The policy of a `--changed` request, or None without it; triggered jobs resolve on `project`.

use crate::consumption::_helpers::policy::DEFINITION;
use crate::consumption::_helpers::policy::REACH_ALL;
use crate::consumption::_helpers::policy::REACH_JOB;
use crate::consumption::_helpers::policy::change_kind;
use crate::consumption::_helpers::policy::selectors;
use crate::consumption::_helpers::policy::triggered_job;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::project::configured_changes::configured_changes;
use sqlbuild_python_project::compiled_project::main::triggered_jobs::triggered_jobs;
use sqlbuild_python_project::graph::models::NativeProjectGraph;
use sqlbuild_state::consumption::models::{ChangeKind, ChangedPolicy, Reach};
use std::collections::BTreeSet;

/// The policy of a `--changed` request, or None without it; triggered jobs resolve on `project`.
pub fn changed_policy(
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
                triggered_jobs(project.py(), graph.graph(), &selectors(request)?)?
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
