use crate::graph::errors::SelectorError;
use crate::python_selection::_helpers::unified::node_selection;
use crate::python_selection::models::{
    PythonSelectionGraph, RunSelection, SelectionContext, SqlSelectionGraph,
};

/// The SQL resources and Python nodes, checks included, that selectors name, as grants and sensor conditions read them.
pub fn resolve_python_sql_nodes(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
    select: &[String],
    exclude: &[String],
) -> Result<RunSelection, SelectorError> {
    node_selection(&SelectionContext::new(sql, python), select, exclude)
}
