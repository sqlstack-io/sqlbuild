//! The layout of the state table `kind` names: `model`, `column` or `old_name`.

use crate::state_rows::constants::{COLUMN_KIND, MODEL_KIND, OLD_NAME_KIND};
use crate::state_rows::models::{EventTable, EventTableLayout};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_state::events::constants::{
    COLUMN_MIGRATION_COLUMNS, MIGRATION_COLUMNS, OLD_NAME_VIEW_COLUMNS,
};

/// The layout of the state table `kind` names: `model`, `column` or `old_name`.
pub fn layout(kind: &str) -> PyResult<EventTableLayout> {
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
