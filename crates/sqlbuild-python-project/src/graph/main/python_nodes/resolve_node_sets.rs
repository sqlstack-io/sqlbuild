//! Python nodes' `job(...)` and `tag(...)` dependencies, resolved once a compile has the SQL graph.

use std::collections::BTreeMap;

use crate::graph::_helpers::python_nodes::python_node_graph::explicit_python_node_graph;
use crate::graph::_helpers::python_nodes::python_selection::{
    allows_round_trips, project_sources, selection_graph,
};
use crate::graph::_helpers::python_nodes::set_dependencies::dependants;
use crate::graph::models::NativeProjectGraph;
use crate::job_settings::main::declared_jobs::declared_jobs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::graph::models::JobSelection;
use sqlbuild_analysis::node_set_dependencies::main::resolve_node_set_dependencies::resolve_node_set_dependencies;
use sqlbuild_analysis::node_set_dependencies::models::DependantNode;
use sqlbuild_analysis::python_selection::models::{PythonSelectionGraph, SqlSelectionGraph};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::NodeSetOutcome;
use sqlbuild_python_front::discovery::main::records::keep_node_sets::keep_node_sets;

/// Resolve and keep the set dependencies of `discovered` over `project`'s graph, errors included.
pub fn resolve_node_sets(
    discovered: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Option<NodeSetOutcome>> {
    let found: Vec<DependantNode> = dependants(discovered)?;
    let outcome: Option<NodeSetOutcome> = if found.is_empty() {
        None
    } else {
        let native: Bound<'_, NativeProjectGraph> =
            native_field(project, "lineage_graph")?.cast_into()?;
        let sources: Vec<(String, Option<String>)> = project_sources(project)?;
        let python: PythonSelectionGraph =
            selection_graph(&explicit_python_node_graph(discovered)?)?;
        let project_dir: Bound<'_, PyAny> = native_field(discovered, "project_dir")?;
        let catalog: BTreeMap<String, JobSelection> = if project_dir.is_none() {
            BTreeMap::new()
        } else {
            declared_jobs(&project_dir)?
        };
        let allow_warehouse_round_trips: bool = allows_round_trips(project)?;
        Some(compiler_guard(|| {
            Ok(resolve_node_set_dependencies(
                SqlSelectionGraph {
                    graph: native.get().graph(),
                    sources: &sources,
                    allow_warehouse_round_trips,
                },
                &python,
                &found,
                &catalog,
            ))
        })?)
    };
    keep_node_sets(discovered, outcome.clone())?;
    Ok(outcome)
}
