//! Create-if-missing DDL whose column types the adapter renders.

use crate::state_rows::_helpers::tables::state_tables;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Create-if-missing DDL whose column types the adapter renders.
#[inline]
pub fn state_table_create_sql<'py>(
    qualified_name: &str,
    (columns, column_types, required_columns): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    render_framework_type: &Bound<'py, PyAny>,
) -> PyResult<String> {
    state_tables::state_table_create_sql(
        qualified_name,
        (columns, column_types, required_columns),
        render_framework_type,
    )
}
