//! Current-state projections over recorded migration events.

use crate::state_rows::main::migrations::newest_column::newest_column;
use crate::state_rows::main::migrations::newest_move::newest_move;
use crate::state_rows::main::migrations::old_name_views::old_name_views;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_state::events::models::OldNameStep;

use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn collected<'py>(events: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    events.try_iter()?.collect()
}

pub(crate) fn chosen<'py>(
    py: Python<'py>,
    events: &[Bound<'py, PyAny>],
    index: Option<usize>,
) -> Bound<'py, PyAny> {
    index
        .and_then(|index| events.get(index).cloned())
        .unwrap_or_else(|| py.None().into_bound(py))
}

pub(crate) fn step(fact: &Bound<'_, PyAny>) -> PyResult<OldNameStep> {
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

#[pyfunction(name = "newest_migration_event_mentioning")]
#[pyo3(signature = (*, events, relation))]
pub(crate) fn newest_move_binding<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    newest_move(events, relation)
}

#[pyfunction(name = "newest_column_migration_event_mentioning")]
#[pyo3(signature = (*, events, relation, origin_column, destination_column))]
pub(crate) fn newest_column_binding<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    origin_column: &str,
    destination_column: &str,
) -> PyResult<Bound<'py, PyAny>> {
    newest_column(events, relation, (origin_column, destination_column))
}

#[pyfunction(name = "project_old_name_views")]
#[pyo3(signature = (*, moves, facts))]
pub(crate) fn old_name_views_binding<'py>(
    moves: &Bound<'py, PyAny>,
    facts: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    old_name_views(moves, facts)
}
