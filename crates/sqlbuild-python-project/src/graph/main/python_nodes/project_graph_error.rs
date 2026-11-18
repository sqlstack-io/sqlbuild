//! `project_graph_error(project_graph, discovered_inputs)`: the first whole-graph boundary error.

use crate::graph::_helpers::python_nodes::python_selection::{
    allows_round_trips, project_sources, selection_graph,
};
use crate::graph::main::python_nodes::python_node_graph::python_node_graph;
use crate::graph::main::python_nodes::resolve_node_sets::resolve_node_sets;
use crate::graph::models::{CompiledProjectGraph, NativeProjectGraph};
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::python_selection::main::python_sql_graph_error::python_sql_graph_error;
use sqlbuild_analysis::python_selection::models::{PythonSelectionGraph, SqlSelectionGraph};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::types::NodeSetOutcome;
use sqlbuild_python_front::discovery::main::records::stored_node_sets::stored_node_sets;

/// The project's first whole-graph boundary error across SQL resources and Python nodes, as every selection reports it.
pub fn project_graph_error(
    project_graph: &Bound<'_, PyAny>,
    discovered_inputs: &Bound<'_, PyAny>,
) -> PyResult<Option<SelectorError>> {
    let py: Python<'_> = project_graph.py();
    let project_graph: &Bound<'_, CompiledProjectGraph> = project_graph.cast()?;
    let project: &Bound<'_, PyAny> = project_graph.get().project(py);
    let sets: Option<NodeSetOutcome> = match stored_node_sets(discovered_inputs)? {
        Some(outcome) => Some(outcome),
        None => resolve_node_sets(discovered_inputs, project)?,
    };
    if let Some(Err(error)) = sets {
        return Ok(Some(error));
    }
    let sources: Vec<(String, Option<String>)> = project_sources(project)?;
    let nodes: PythonSelectionGraph = selection_graph(&python_node_graph(discovered_inputs)?)?;
    let native: &Bound<'_, NativeProjectGraph> = project_graph.get().native(py);
    compiler_guard(|| {
        Ok(python_sql_graph_error(
            SqlSelectionGraph {
                graph: native.get().graph(),
                sources: &sources,
                allow_warehouse_round_trips: allows_round_trips(project)?,
            },
            &nodes,
        ))
    })
}
