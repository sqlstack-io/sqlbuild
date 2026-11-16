//! The adapter's qualified name of a state table in `(database, schema)`, if it has one.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The adapter's qualified name of a state table in `(database, schema)`, if it has one.
#[inline]
pub fn state_table_name(
    render_qualified_name: &Bound<'_, PyAny>,
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    table: &str,
) -> PyResult<Option<String>> {
    consumption_state::state_table_name(render_qualified_name, (database, schema), table)
}
