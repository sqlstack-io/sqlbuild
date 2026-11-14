use std::collections::BTreeMap;

use crate::graph::main::attach_jobs_with::attach_jobs_with;
use crate::graph::models::{JobSelection, ProjectGraph};
use crate::python_selection::_helpers::unified::run_selection;
use crate::python_selection::models::{
    PythonSelectionGraph, RunSelection, SelectionContext, SqlSelectionGraph,
};

/// Attach jobs as publication resolves them, keeping each job's Python nodes as `(kind, name)` keys.
pub fn attach_python_jobs(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
    catalog: BTreeMap<String, JobSelection>,
) -> ProjectGraph {
    let kinds: BTreeMap<&str, &str> = python
        .nodes
        .iter()
        .map(|node| (node.name.as_str(), node.kind.as_str()))
        .collect();
    attach_jobs_with(sql.graph.clone(), catalog, |graph, select, exclude| {
        let context: SelectionContext<'_> = SelectionContext::new(
            SqlSelectionGraph {
                graph,
                sources: sql.sources,
            },
            python,
        );
        let RunSelection {
            sql_keys,
            python_node_names,
        } = run_selection(&context, select, exclude)?;
        let python_keys = python_node_names
            .into_iter()
            .filter_map(|name| Some((kinds.get(name.as_str())?.to_string(), name)));
        Ok(sql_keys.into_iter().chain(python_keys).collect())
    })
}
