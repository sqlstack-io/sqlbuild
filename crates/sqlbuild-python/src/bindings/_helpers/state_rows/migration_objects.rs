//! Migration state table layouts and the Python objects stored in them.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_state::events::constants::{
    COLUMN_MIGRATION_COLUMNS, MIGRATION_COLUMNS, OLD_NAME_VIEW_COLUMNS,
};
use sqlbuild_state::events::models::EventOrder;

use crate::bindings::_helpers::state_rows::constants::{COLUMN_KIND, MODEL_KIND, OLD_NAME_KIND};
use crate::bindings::_helpers::state_rows::models::{EventTable, EventTableLayout};
use crate::bindings::_helpers::state_rows::timestamps::presumed_utc;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// The layout of the state table `kind` names: `model`, `column` or `old_name`.
pub(crate) fn layout(kind: &str) -> PyResult<EventTableLayout> {
    match kind {
        MODEL_KIND => Ok(EventTableLayout {
            table: EventTable::Model,
            name: MIGRATION_TABLE_NAME,
            columns: &MIGRATION_COLUMNS,
            relation_attribute: "destination",
            state_message: "model migration state requires a target schema",
            event_message: "model migration events require a destination schema",
            read_subject: "model migration events",
        }),
        COLUMN_KIND => Ok(EventTableLayout {
            table: EventTable::Column,
            name: COLUMN_MIGRATION_TABLE_NAME,
            columns: &COLUMN_MIGRATION_COLUMNS,
            relation_attribute: "relation",
            state_message: "column migration state requires a target schema",
            event_message: "column migration events require a relation schema",
            read_subject: "column migration events",
        }),
        OLD_NAME_KIND => Ok(EventTableLayout {
            table: EventTable::OldName,
            name: OLD_NAME_VIEW_TABLE_NAME,
            columns: &OLD_NAME_VIEW_COLUMNS,
            relation_attribute: "new",
            state_message: "old-name view state requires a target schema",
            event_message: "old-name view facts require a destination schema",
            read_subject: "old-name view facts",
        }),
        _ => Err(PyValueError::new_err(format!(
            "unknown migration state table kind: {kind}"
        ))),
    }
}

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
