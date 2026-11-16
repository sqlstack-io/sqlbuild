//! Resolve graph keys for native planning without building a public frozenset.

use crate::compiled_project::main::graph_with_jobs::graph_with_jobs;
use crate::planner_selection::_helpers::selectors::graph_names;
use crate::planner_selection::main::py_strings::py_strings;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::ProjectGraph;

/// Resolve graph keys for native planning without building a public frozenset.
pub fn resolve_graph_keys<'py>(
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Vec<ObjectKey>> {
    let py: Python<'py> = select.py();
    let select: Vec<Bound<'py, PyString>> = py_strings(select)?;
    let exclude: Vec<Bound<'py, PyString>> = py_strings(exclude)?;
    let texts: Vec<Bound<'py, PyString>> = select.iter().chain(&exclude).cloned().collect();
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || Ok(graph_names(graph)))?;
    let (select, exclude) = (carrier.send_all(&select)?, carrier.send_all(&exclude)?);
    let all: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let graph = graph_with_jobs(py, graph, &all, &[])?;
    resolve_selectors(&graph, &select, &exclude).map_err(|error| carrier.error(error))
}
