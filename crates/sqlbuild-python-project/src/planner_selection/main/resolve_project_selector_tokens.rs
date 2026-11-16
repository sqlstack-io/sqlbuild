//! The keys whitespace-separated selector tokens match, without required build resources.

use crate::compiled_project::main::graph_with_jobs::graph_with_jobs;
use crate::graph::models::NativeProjectGraph;
use crate::planner_selection::_helpers::selectors::VIEW_OPTIONS;
use crate::planner_selection::_helpers::selectors::arguments_graph;
use crate::planner_selection::_helpers::selectors::graph_names;
use crate::planner_selection::_helpers::selectors::selected;
use crate::planner_selection::main::py_strings::py_strings;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyString};
use sqlbuild_analysis::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// The keys whitespace-separated selector tokens match, without required build resources.
pub fn resolve_project_selector_tokens<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_project_selector_tokens",
        options,
        (&[&["selectors"], VIEW_OPTIONS].concat(), &[]),
    )?;
    let graph: NativeProjectGraph = arguments_graph(&arguments)?;
    let texts: Vec<Bound<'py, PyString>> = py_strings(&arguments.value("selectors")?)?;
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || Ok(graph_names(graph.graph())))?;
    let selectors: Vec<String> = carrier.send_all(&texts)?;
    let graph = graph_with_jobs(py, graph.graph(), &selectors, &[])?;
    selected(&carrier, resolve_selector_tokens(&graph, &selectors))
}
