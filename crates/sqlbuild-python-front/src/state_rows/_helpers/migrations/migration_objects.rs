//! Migration state table layouts and the Python objects stored in them.

use crate::state_rows::main::event_rows::presumed_utc::presumed_utc;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_state::events::models::EventOrder;

use crate::state_rows::models::EventTableLayout;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// `MigrationStateError(message)`.
pub(crate) fn state_error(py: Python<'_>, message: &str) -> PyErr {
    new_plain_error(py, ErrorId::MigrationState, message)
}

/// The adapter-qualified name of `layout`'s table in one schema.
pub(crate) fn qualified_table<'py>(
    layout: &EventTableLayout,
    render_qualified_name: &Bound<'py, PyAny>,
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<String> {
    let py: Python<'py> = render_qualified_name.py();
    let table: Bound<'py, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database),
            ("schema", schema),
            ("name", PyString::new(py, layout.name).into_any()),
        ],
    )?;
    if table.is_none() {
        return Err(state_error(py, layout.state_message));
    }
    table.extract()
}

/// The history order of a stored event: its creation time in UTC, then its ID.
pub(crate) fn event_order(event: &Bound<'_, PyAny>) -> PyResult<EventOrder> {
    Ok(EventOrder {
        created_at: presumed_utc(&native_field(event, "created_at")?)?,
        event_id: native_field(event, "event_id")?.extract()?,
    })
}
