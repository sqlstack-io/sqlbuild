//! Create-if-missing DDL for a state table with fixed columns, typed by the adapter.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Create-if-missing DDL for a state table with fixed columns, typed by the adapter.
#[inline]
pub fn fixed_state_table_sql(
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (render_qualified_name, render_framework_type): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (table_name, column_types, nullable): (&str, &[(&str, &str)], &[&str]),
) -> PyResult<String> {
    consumption_state::fixed_state_table_sql(
        (database, schema),
        (render_qualified_name, render_framework_type),
        (table_name, column_types, nullable),
    )
}
