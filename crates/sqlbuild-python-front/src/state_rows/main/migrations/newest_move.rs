//! The latest recorded move naming `relation` as origin or destination, or `None`.

use crate::state_rows::_helpers::event_rows::events::relation as event_relation;
use crate::state_rows::_helpers::migrations::migration_objects::event_order;
use crate::state_rows::_helpers::migrations::migration_projections::chosen;
use crate::state_rows::_helpers::migrations::migration_projections::collected;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::events::main::newest_move_event::newest_move_event;
use sqlbuild_state::events::models::MoveEventKey;

/// The latest recorded move naming `relation` as origin or destination, or `None`.
pub fn newest_move<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let events: Vec<Bound<'py, PyAny>> = collected(events)?;
    let keys: Vec<MoveEventKey> = events
        .iter()
        .map(|event| {
            Ok(MoveEventKey {
                order: event_order(event)?,
                origin: event_relation(&native_field(event, "origin")?)?,
                destination: event_relation(&native_field(event, "destination")?)?,
            })
        })
        .collect::<PyResult<_>>()?;
    let index: Option<usize> = newest_move_event(&keys, &event_relation(relation)?);
    Ok(chosen(relation.py(), &events, index))
}
