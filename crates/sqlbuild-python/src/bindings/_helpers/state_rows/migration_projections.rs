//! Current-state projections over recorded migration events.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_state::events::main::newest_column_event::newest_column_event;
use sqlbuild_state::events::main::newest_move_event::newest_move_event;
use sqlbuild_state::events::main::old_name_histories::old_name_histories;
use sqlbuild_state::events::models::{
    ColumnEventKey, EventOrder, EventRelation, MoveEventKey, OldNameFactKey, OldNameStep,
};

use crate::bindings::_helpers::state_rows::constants::OLD_NAME_VIEW_CONFIG_KEY;
use crate::bindings::_helpers::state_rows::events::relation as event_relation;
use crate::bindings::_helpers::state_rows::migration_objects::event_order;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn collected<'py>(events: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    events.try_iter()?.collect()
}

fn chosen<'py>(
    py: Python<'py>,
    events: &[Bound<'py, PyAny>],
    index: Option<usize>,
) -> Bound<'py, PyAny> {
    index
        .and_then(|index| events.get(index).cloned())
        .unwrap_or_else(|| py.None().into_bound(py))
}

/// The latest recorded move naming `relation` as origin or destination, or `None`.
pub(crate) fn newest_move<'py>(
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

/// The latest recorded rename on `relation` naming either column, or `None`.
pub(crate) fn newest_column<'py>(
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

fn step(fact: &Bound<'_, PyAny>) -> PyResult<OldNameStep> {
    let value: String = native_field(&native_field(fact, "event_type")?, "value")?.extract()?;
    match value.as_str() {
        "required" => Ok(OldNameStep::Required),
        "origin_archived" => Ok(OldNameStep::OriginArchived),
        "view_created" => Ok(OldNameStep::ViewCreated),
        "view_dropped" => Ok(OldNameStep::ViewDropped),
        _ => Err(PyValueError::new_err(format!(
            "unknown old-name view event type: {value}"
        ))),
    }
}

/// One history per recorded move whose old name needs a compatibility view, in move order.
pub(crate) fn old_name_views<'py>(
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

/// The model's old-name view retention, the project's when unset, or `None` when disabled.
pub(crate) fn old_name_retention<'py>(
    config_values: &Bound<'py, PyAny>,
    project_retention: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = config_values.py();
    if !config_values.contains(OLD_NAME_VIEW_CONFIG_KEY)? {
        return Ok(project_retention.clone());
    }
    let value: Bound<'py, PyAny> = config_values.get_item(OLD_NAME_VIEW_CONFIG_KEY)?;
    if value.is(PyBool::new(py, false)) {
        return Ok(py.None().into_bound(py));
    }
    value.str()?.call_method0("strip")
}

#[pyfunction(name = "newest_migration_event_mentioning")]
#[pyo3(signature = (*, events, relation))]
fn newest_move_binding<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    newest_move(events, relation)
}

#[pyfunction(name = "newest_column_migration_event_mentioning")]
#[pyo3(signature = (*, events, relation, origin_column, destination_column))]
fn newest_column_binding<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    origin_column: &str,
    destination_column: &str,
) -> PyResult<Bound<'py, PyAny>> {
    newest_column(events, relation, (origin_column, destination_column))
}

#[pyfunction(name = "project_old_name_views")]
#[pyo3(signature = (*, moves, facts))]
fn old_name_views_binding<'py>(
    moves: &Bound<'py, PyAny>,
    facts: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    old_name_views(moves, facts)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(newest_move_binding, module)?)?;
    module.add_function(wrap_pyfunction!(newest_column_binding, module)?)?;
    module.add_function(wrap_pyfunction!(old_name_views_binding, module)?)?;
    Ok(())
}
