use crate::graph::errors::SelectorError;
use crate::python_selection::_helpers::unified::run_selection;
use crate::python_selection::models::{
    PythonSelectionGraph, RunSelection, SelectionContext, SqlSelectionGraph,
};

/// Python's `resolve_python_sql_run_selectors`: run selectors over SQL resources and Python nodes.
pub fn resolve_python_sql_run_selection(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
    select: &[String],
    exclude: &[String],
) -> Result<RunSelection, SelectorError> {
    run_selection(&SelectionContext::new(sql, python), select, exclude)
}
