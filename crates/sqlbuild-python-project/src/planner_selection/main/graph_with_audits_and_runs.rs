//! The graph a command's selectors resolve over, with the project's audits and the past runs they name.

use std::borrow::Cow;

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::models::ProjectGraph;

use crate::planner_selection::_helpers::past_runs::with_audits_and_runs;

/// `graph` with `project`'s audits and the past runs attached when `selectors` name an audit or a run and the command reads them.
pub fn graph_with_audits_and_runs<'g>(
    project: &Bound<'_, PyAny>,
    graph: &'g ProjectGraph,
    selectors: &[String],
) -> PyResult<Result<Cow<'g, ProjectGraph>, SelectorError>> {
    with_audits_and_runs(project, graph, selectors)
}
