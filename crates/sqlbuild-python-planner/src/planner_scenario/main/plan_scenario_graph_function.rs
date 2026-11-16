//! `plan_scenario_graph` as a function of `module`.

use crate::planner_scenario::_helpers::graph::plan_scenario_graph;

/// `plan_scenario_graph` as a function of `module`.
pub fn plan_scenario_graph_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(plan_scenario_graph, module)
}
