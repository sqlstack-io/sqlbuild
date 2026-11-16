//! Raise S405 when a selected resource reads an external source table that does not exist.

use crate::planner_warehouse::_helpers::sources::source_tables;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Raise S405 when a selected resource reads an external source table that does not exist.
#[inline]
pub fn check_selected_source_tables_exist<'py>(
    project: &Bound<'py, PyAny>,
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<()> {
    source_tables::check_selected_source_tables_exist(
        project,
        (adapter, connection),
        scope,
        relations,
    )
}
