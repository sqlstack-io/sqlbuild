use crate::graph::errors::SelectorError;
use crate::python_selection::_helpers::nodes::sql_names;
use crate::python_selection::_helpers::unified::node_selection;
use crate::python_selection::models::{
    NodeIndex, PythonSelectionGraph, RunSelection, SelectionContext, SqlSelectionGraph,
};

/// The SQL resources and Python nodes, checks included, that selectors name, as grants and sensor conditions read them.
pub fn resolve_python_sql_nodes(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
    select: &[String],
    exclude: &[String],
) -> Result<RunSelection, SelectorError> {
    let context: SelectionContext<'_> = SelectionContext {
        sql,
        nodes: NodeIndex::new(python),
        sql_names: sql_names(sql.graph),
    };
    node_selection(&context, select, exclude)
}
