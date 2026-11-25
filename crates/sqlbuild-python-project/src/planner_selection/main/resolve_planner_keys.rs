//! The keys and audits a command's selectors pick for planning, with the project's audits and the past runs they name.

use std::collections::BTreeSet;

use crate::planner_selection::_helpers::selectors::resolve_graph_keys;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::AUDIT_KIND;
use sqlbuild_analysis::graph::models::ProjectGraph;

/// The graph keys `select` and `exclude` pick, and apart from them the names of the audits they pick.
pub fn resolve_planner_keys<'py>(
    project: &Bound<'py, PyAny>,
    graph: &ProjectGraph,
    selectors: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Vec<ObjectKey>, BTreeSet<String>)> {
    let (audits, nodes): (Vec<ObjectKey>, Vec<ObjectKey>) =
        resolve_graph_keys(Some(project), graph, selectors)?
            .into_iter()
            .partition(|key| key.0 == AUDIT_KIND);
    Ok((nodes, audits.into_iter().map(|(_, name)| name).collect()))
}
