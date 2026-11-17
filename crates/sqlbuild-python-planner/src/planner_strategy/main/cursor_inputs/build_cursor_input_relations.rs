//! Cursor-bearing input relations of an incremental model, for runtime range discovery.

use crate::planner_strategy::_helpers::cursor_inputs;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_project::planner_strategy::models::CursorInputLookups;

/// Cursor-bearing input relations of an incremental model, for runtime range discovery.
#[inline]
pub fn build_cursor_input_relations<'py>(
    (model, dialect): (&Bound<'py, PyAny>, &Bound<'py, Dialect>),
    cursor_column: &Bound<'py, PyAny>,
    lookups: &CursorInputLookups<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    cursor_inputs::build_cursor_input_relations((model, dialect), cursor_column, lookups)
}
