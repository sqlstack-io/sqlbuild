//! The keys and audits a command's selectors pick for planning, with the project's audits and the past runs they name.

use std::collections::BTreeSet;

use crate::job_settings::main::graph_with_jobs::graph_with_jobs;
use crate::planner_selection::_helpers::past_runs::with_audits_and_runs;
use crate::planner_selection::_helpers::selectors::graph_names;
use crate::planner_selection::main::py_strings::py_strings;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::AUDIT_KIND;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::ProjectGraph;

/// The graph keys `select` and `exclude` pick, and apart from them the names of the audits they pick.
pub fn resolve_planner_keys<'py>(
    project: &Bound<'py, PyAny>,
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Vec<ObjectKey>, BTreeSet<String>)> {
    let py: Python<'py> = select.py();
    let select: Vec<Bound<'py, PyString>> = py_strings(select)?;
    let exclude: Vec<Bound<'py, PyString>> = py_strings(exclude)?;
    let texts: Vec<Bound<'py, PyString>> = select.iter().chain(&exclude).cloned().collect();
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || Ok(graph_names(graph)))?;
    let (select, exclude) = (carrier.send_all(&select)?, carrier.send_all(&exclude)?);
    let all: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let graph = graph_with_jobs(py, graph, &all, &[])?;
    let graph =
        with_audits_and_runs(project, &graph, &all)?.map_err(|error| carrier.error(error))?;
    let keys: Vec<ObjectKey> =
        resolve_selectors(&graph, &select, &exclude).map_err(|error| carrier.error(error))?;
    let (audits, nodes): (Vec<ObjectKey>, Vec<ObjectKey>) =
        keys.into_iter().partition(|key| key.0 == AUDIT_KIND);
    Ok((nodes, audits.into_iter().map(|(_, name)| name).collect()))
}
