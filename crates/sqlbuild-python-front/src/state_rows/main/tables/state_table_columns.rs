//! A state table's columns with the types the adapter renders.

use crate::state_rows::_helpers::tables::state_tables;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_state::state_tables::models::TableColumn;

/// Each column with the type the adapter renders for it, and whether it is required.
#[inline]
pub fn state_table_columns<'py>(
    (columns, column_types, required_columns): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    render_framework_type: &Bound<'py, PyAny>,
) -> PyResult<Vec<TableColumn>> {
    state_tables::state_table_columns(
        (columns, column_types, required_columns),
        render_framework_type,
    )
}
