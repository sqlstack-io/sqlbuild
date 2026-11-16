//! Migration event IDs and the typed literals of one appended event row.

use crate::state_rows::main::event_rows::date_time_typed::date_time_typed;
use crate::state_rows::main::event_rows::presumed_utc::presumed_utc;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::events::errors::StateSqlError;
use sqlbuild_state::events::main::column_migration_event_id::column_migration_event_id;
use sqlbuild_state::events::main::event_cells::event_cells;
use sqlbuild_state::events::main::event_insert_sql::event_insert_sql;
use sqlbuild_state::events::main::migration_event_id::migration_event_id;
use sqlbuild_state::events::main::old_name_view_event_id::old_name_view_event_id;
use sqlbuild_state::events::models::{EventRelation, StateCell, StateColumnType, StateValue};

use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

fn emit_error(error: JsonEmitError) -> PyErr {
    PyValueError::new_err(format!("{error:?}"))
}

pub(crate) fn relation(value: &Bound<'_, PyAny>) -> PyResult<EventRelation> {
    Ok(EventRelation {
        database: native_field(value, "database")?.extract()?,
        schema: native_field(value, "schema")?.extract()?,
        name: native_field(value, "name")?.extract()?,
    })
}

/// The content-addressed ID for one migration performed by one run.
#[pyfunction(name = "deterministic_migration_event_id")]
#[pyo3(signature = (*, run_id, target_name, origin, destination))]
pub(crate) fn migration_id(
    run_id: &str,
    target_name: Option<&str>,
    origin: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<String> {
    migration_event_id(
        (run_id, target_name),
        &relation(origin)?,
        &relation(destination)?,
    )
    .map_err(emit_error)
}

/// The content-addressed ID for one column rename performed by one run.
#[pyfunction(name = "deterministic_column_migration_event_id")]
#[pyo3(signature = (**request))]
pub(crate) fn column_id(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("deterministic_column_migration_event_id", request)?;
    let run_id: String = request.get("run_id")?.extract()?;
    let target_name: Option<String> = request.get("target_name")?.extract()?;
    let origin_column: String = request.get("origin_column")?.extract()?;
    let destination_column: String = request.get("destination_column")?.extract()?;
    column_migration_event_id(
        (&run_id, target_name.as_deref()),
        &relation(&request.get("relation")?)?,
        (&origin_column, &destination_column),
    )
    .map_err(emit_error)
}

/// The content-addressed ID of one old-name step of one recorded move.
#[pyfunction(name = "deterministic_old_name_view_event_id")]
#[pyo3(signature = (*, event_type, migration_event_id))]
pub(crate) fn old_name_id(
    event_type: &Bound<'_, PyAny>,
    migration_event_id: &str,
) -> PyResult<String> {
    let event_type: String = native_field(event_type, "value")?.extract()?;
    old_name_view_event_id(&event_type, migration_event_id).map_err(emit_error)
}

fn state_value(value: &Bound<'_, PyAny>, column_type: StateColumnType) -> PyResult<StateValue> {
    if value.is_none() {
        return Ok(StateValue::Null);
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(StateValue::Text(text.extract()?));
    }
    if column_type == StateColumnType::TextTimestamp && date_time_typed(value)? {
        return Ok(StateValue::UtcTimestamp(presumed_utc(value)?));
    }
    Ok(StateValue::Other)
}

/// One append-only event row as `INSERT ... VALUES` with typed literals, failing like Python.
pub(crate) fn typed_insert_sql(
    py: Python<'_>,
    table: &str,
    (columns, values): (&[&str], &[Bound<'_, PyAny>]),
) -> PyResult<String> {
    let mut cells: Vec<StateCell> = event_cells(columns, vec![StateValue::Null; values.len()]);
    for (index, value) in values.iter().enumerate() {
        match state_value(value, cells[index].column_type) {
            Ok(converted) => cells[index].value = converted,
            Err(error) => {
                event_insert_sql(table, &cells[..index]).map_err(rendering_error(py))?;
                return Err(error);
            }
        }
    }
    event_insert_sql(table, &cells).map_err(rendering_error(py))
}

/// `SqlValueRenderingError(message)` for an ill-typed value.
fn rendering_error(py: Python<'_>) -> impl Fn(StateSqlError) -> PyErr + '_ {
    move |error: StateSqlError| {
        let raised = host_entry(py, ExecutorHost::SqlValueRenderingError)
            .and_then(|class| class.call1((error.0,)));
        match raised {
            Ok(exception) => PyErr::from_value(exception),
            Err(failure) => failure,
        }
    }
}
