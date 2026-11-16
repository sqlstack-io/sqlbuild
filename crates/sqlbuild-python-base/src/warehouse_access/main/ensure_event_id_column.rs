//! Missing callbacks retain the existing renderer-only API for already-current/new schemas.

use crate::warehouse_access::_helpers::state_column_evolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Missing callbacks retain the existing renderer-only API for already-current/new schemas.
#[inline]
pub fn ensure_event_id_column<'py>(
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (table, qualified): (&str, &str),
    (read_columns, render_add, render_type): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<()> {
    state_column_evolution::ensure_event_id_column(
        (connection, execute),
        (database, schema),
        (table, qualified),
        (read_columns, render_add, render_type),
    )
}
