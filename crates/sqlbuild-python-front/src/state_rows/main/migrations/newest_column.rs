//! The latest recorded rename on `relation` naming either column, or `None`.

use crate::state_rows::_helpers::event_rows::events::relation as event_relation;
use crate::state_rows::_helpers::migrations::migration_objects::event_order;
use crate::state_rows::_helpers::migrations::migration_projections::chosen;
use crate::state_rows::_helpers::migrations::migration_projections::collected;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::events::main::newest_column_event::newest_column_event;
use sqlbuild_state::events::models::{ColumnEventKey, EventRelation};

/// The latest recorded rename on `relation` naming either column, or `None`.
pub fn newest_column<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    (origin_column, destination_column): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let events: Vec<Bound<'py, PyAny>> = collected(events)?;
    let keys: Vec<ColumnEventKey> = events
        .iter()
        .map(|event| {
            Ok(ColumnEventKey {
                order: event_order(event)?,
                relation: event_relation(&native_field(event, "relation")?)?,
                origin_column: native_field(event, "origin_column")?.extract()?,
                destination_column: native_field(event, "destination_column")?.extract()?,
            })
        })
        .collect::<PyResult<_>>()?;
    let target: EventRelation = event_relation(relation)?;
    let index: Option<usize> =
        newest_column_event(&keys, &target, (origin_column, destination_column));
    Ok(chosen(relation.py(), &events, index))
}
