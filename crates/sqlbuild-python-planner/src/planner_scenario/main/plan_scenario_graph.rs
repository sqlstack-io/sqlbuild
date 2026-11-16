//! Plan scenario graph.

use crate::planner_scenario::_helpers::graph;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

/// Plan scenario graph.
#[inline]
pub fn plan_scenario_graph<'py>(
    scenario: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    sql_analysis_enabled: bool,
    sql_analysis_dialect: Option<&str>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>)> {
    graph::plan_scenario_graph(
        scenario,
        project,
        sql_analysis_enabled,
        sql_analysis_dialect,
    )
}
