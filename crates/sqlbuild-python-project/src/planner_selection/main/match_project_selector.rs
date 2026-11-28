//! The keys one parsed selector matches before `+` graph expansion.

use crate::graph::models::GraphViewMappings;
use crate::graph::models::NativeProjectGraph;
use crate::job_settings::main::graph_with_jobs::graph_with_jobs;
use crate::planner_selection::_helpers::selectors::graph_names;
use crate::planner_selection::_helpers::selectors::selected;
use crate::planner_selection::_helpers::selectors::view_graph;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyString};
use sqlbuild_analysis::graph::main::match_selector::match_selector;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The keys one parsed selector matches before `+` graph expansion.
pub fn match_project_selector<'py>(
    parsed: &Bound<'py, PyAny>,
    all_keys: &Bound<'py, PyAny>,
    tag_index: &Bound<'py, PyAny>,
    path_index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = parsed.py();
    let empty: Bound<'py, PyAny> = PyDict::new(py).into_any();
    let graph: NativeProjectGraph = view_graph(GraphViewMappings {
        all_keys,
        upstream: &empty,
        downstream: &empty,
        tag_index,
        path_index,
    })?;
    let value: Bound<'py, PyString> = native_field(parsed, "value")?.cast_into()?;
    let carrier: Carrier<'py> = Carrier::new(py, std::slice::from_ref(&value), || {
        Ok(graph_names(graph.graph()))
    })?;
    let kind: String = native_field(parsed, "kind")?.str()?.extract()?;
    let value: String = carrier.send(&value)?;
    let graph = graph_with_jobs(py, graph.graph(), &[format!("{kind}:{value}")], &[])?;
    selected(
        &carrier,
        match_selector(&graph, &kind, &value).map(|keys| keys.into_iter().collect()),
    )
}
