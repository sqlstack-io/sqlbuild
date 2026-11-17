//! One history per recorded move whose old name needs a compatibility view, in move order.

use crate::state_rows::_helpers::migrations::migration_objects::event_order;
use crate::state_rows::_helpers::migrations::migration_projections::chosen;
use crate::state_rows::_helpers::migrations::migration_projections::collected;
use crate::state_rows::_helpers::migrations::migration_projections::step;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_state::events::main::old_name_histories::old_name_histories;
use sqlbuild_state::events::models::{EventOrder, OldNameFactKey};

/// One history per recorded move whose old name needs a compatibility view, in move order.
pub fn old_name_views<'py>(
    moves: &Bound<'py, PyAny>,
    facts: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = moves.py();
    let moves: Vec<Bound<'py, PyAny>> = collected(moves)?;
    let facts: Vec<Bound<'py, PyAny>> = collected(facts)?;
    let move_orders: Vec<EventOrder> = moves.iter().map(event_order).collect::<PyResult<_>>()?;
    let fact_keys: Vec<OldNameFactKey> = facts
        .iter()
        .map(|fact| {
            Ok(OldNameFactKey {
                order: event_order(fact)?,
                migration_event_id: native_field(fact, "migration_event_id")?.extract()?,
                step: step(fact)?,
            })
        })
        .collect::<PyResult<_>>()?;
    let fact_at = |index: Option<usize>| chosen(py, &facts, index);
    let histories: Vec<Bound<'py, PyAny>> = old_name_histories(&move_orders, &fact_keys)
        .into_iter()
        .map(|indexes| {
            new_record_with(
                py,
                RecordId::OldNameViewHistory,
                &[
                    ("move", moves[indexes.moved].clone()),
                    ("required", facts[indexes.required].clone()),
                    ("archived", fact_at(indexes.archived)),
                    ("created", fact_at(indexes.created)),
                    ("dropped", fact_at(indexes.dropped)),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, histories)
}
