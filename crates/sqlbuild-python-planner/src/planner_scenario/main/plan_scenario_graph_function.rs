//! `plan_scenario_graph` as a function of `module`.

use crate::planner_scenario::_helpers::graph;

/// `plan_scenario_graph` as a function of `module`.
#[inline]
pub fn plan_scenario_graph_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    graph::plan_scenario_graph_function(module)
}
