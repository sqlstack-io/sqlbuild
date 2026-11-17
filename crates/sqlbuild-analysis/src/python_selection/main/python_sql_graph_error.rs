//! The first whole-graph boundary error across SQL resources and Python nodes, which compile and selection share.

use crate::graph::errors::SelectorError;
use crate::python_selection::_helpers::validation::validate_boundaries;
use crate::python_selection::models::{PythonSelectionGraph, SelectionContext, SqlSelectionGraph};

/// The error every selection of this graph refuses with, whatever it selects, such as a dependency cycle.
#[must_use]
pub fn python_sql_graph_error(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
) -> Option<SelectorError> {
    validate_boundaries(&SelectionContext::new(sql, python)).err()
}
