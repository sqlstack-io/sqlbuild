//! Read and append migration state events through the adapter's execute callable.

use crate::state_rows::main::event_rows::presumed_utc::presumed_utc;
use crate::state_rows::main::migrations::layout::layout;
use crate::state_rows::main::migrations::read_events::read_events;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString, PyTuple, PyTupleMethods};
use sqlbuild_state::events::main::recorded_renames::recorded_renames;
use sqlbuild_state::events::models::RecordedRenameKey;

use crate::state_rows::_helpers::event_rows::events::relation as event_relation;
use crate::state_rows::_helpers::migrations::migration_objects::{event_order, state_error};
use crate::state_rows::_helpers::migrations::migration_rows::write_statements;
use crate::state_rows::constants::{
    COLUMN_KIND, LOGGING_MODULE, MIGRATION_LOGGER, MIGRATION_WRITE_ATTEMPTS,
    MIGRATION_WRITE_RETRY_BASE_SECONDS, MIGRATION_WRITE_RETRY_MESSAGE, TIME_MODULE,
};
use crate::state_rows::models::{EventTable, EventTableLayout, StateAccess};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn execute<'py>(access: &StateAccess<'py>, sql: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = access.execute.py();
    call_with(
        &access.execute,
        &[
            ("connection", access.connection.clone()),
            ("sql", PyString::new(py, sql).into_any()),
        ],
    )
}

pub(crate) fn unreadable(
    layout: &EventTableLayout,
    schema: &Bound<'_, PyAny>,
    error: PyErr,
) -> PyErr {
    let py: Python<'_> = schema.py();
    let describe = |value: &Bound<'_, PyAny>| -> String {
        value.str().map(|text| text.to_string()).unwrap_or_default()
    };
    let message: String = format!(
        "Unable to read {} from schema '{}': {}",
        layout.read_subject,
        describe(schema),
        describe(error.value(py).as_any()),
    );
    let wrapped: PyErr = state_error(py, &message);
    wrapped.set_cause(py, Some(error));
    wrapped
}

fn subject(table: EventTable, event: &Bound<'_, PyAny>) -> PyResult<String> {
    let text = |value: Bound<'_, PyAny>| -> PyResult<String> { Ok(value.str()?.to_string()) };
    Ok(match table {
        EventTable::Model => text(native_field(event, "destination_model")?)?,
        EventTable::Column => format!(
            "{}.{}",
            text(native_field(event, "model_name")?)?,
            text(native_field(event, "destination_column")?)?
        ),
        EventTable::OldName => format!(
            "{} old name {}",
            text(native_field(event, "destination_model")?)?,
            text(native_field(&native_field(event, "old")?, "name")?)?
        ),
    })
}

fn append_once(
    access: &StateAccess<'_>,
    create_table_sql: Option<&str>,
    (existing_sql, insert_sql): (&str, &str),
) -> PyResult<()> {
    if let Some(create_table_sql) = create_table_sql {
        execute(access, create_table_sql)?;
    }
    if !execute(access, existing_sql)?
        .call_method0("fetchall")?
        .is_truthy()?
    {
        execute(access, insert_sql)?;
    }
    Ok(())
}

fn retry_pause(
    py: Python<'_>,
    (attempt, attempts): (i64, i64),
    subject: &str,
    error: &PyErr,
) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = py
        .import(LOGGING_MODULE)?
        .call_method1("getLogger", (MIGRATION_LOGGER,))?;
    logger.call_method1(
        "warning",
        (
            MIGRATION_WRITE_RETRY_MESSAGE,
            attempt + 1,
            attempts,
            subject,
            error.value(py),
        ),
    )?;
    let step: f64 = f64::from(i32::try_from(attempt + 1).unwrap_or(i32::MAX));
    py.import(TIME_MODULE)?
        .call_method1("sleep", (MIGRATION_WRITE_RETRY_BASE_SECONDS * step,))?;
    Ok(())
}

/// Create the state table when given DDL, then insert the event unless its ID is recorded.
fn append_event(
    access: &StateAccess<'_>,
    (kind, event): (&str, &Bound<'_, PyAny>),
    create_table_sql: Option<&str>,
    attempts: i64,
) -> PyResult<()> {
    let py: Python<'_> = event.py();
    let layout: EventTableLayout = layout(kind)?;
    let (existing_sql, insert_sql): (String, String) =
        write_statements(&layout, event, &access.render_qualified_name)?;
    let subject: String = subject(layout.table, event)?;
    for attempt in 0..attempts {
        let Err(error) = append_once(access, create_table_sql, (&existing_sql, &insert_sql)) else {
            return Ok(());
        };
        if attempt + 1 == attempts || !error.is_instance_of::<PyException>(py) {
            return Err(error);
        }
        retry_pause(py, (attempt, attempts), &subject, &error)?;
    }
    Ok(())
}

pub(crate) fn access<'py>(request: &Request<'_, 'py>) -> PyResult<StateAccess<'py>> {
    Ok(StateAccess {
        connection: request.get("connection")?,
        execute: request.get("execute")?,
        render_qualified_name: request.get("render_qualified_name")?,
    })
}

pub(crate) fn stored_names(stored: Option<Bound<'_, PyAny>>) -> PyResult<Option<Vec<String>>> {
    match stored {
        Some(stored) if !stored.is_none() => stored
            .try_iter()?
            .map(|column| column?.extract())
            .collect::<PyResult<Vec<String>>>()
            .map(Some),
        _ => Ok(None),
    }
}

/// Every event of one migration state table this version reads from one schema.
#[pyfunction(name = "read_migration_state_events")]
#[pyo3(signature = (**request))]
pub(crate) fn read_state_events<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("read_migration_state_events", request)?;
    let stored: Option<Vec<String>> = stored_names(request.optional("stored_columns")?)?;
    read_events(
        &access(&request)?,
        &layout(&request.get("kind")?.extract::<String>()?)?,
        (request.get("database")?, request.get("schema")?),
        stored.as_deref(),
    )
}

/// Renames recorded on the relation at or after `since`, oldest first.
#[pyfunction(name = "read_recorded_column_renames")]
#[pyo3(signature = (**request))]
pub(crate) fn read_renames<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("read_recorded_column_renames", request)?;
    let py: Python<'py> = request.py();
    let relation: Bound<'py, PyAny> = request.get("relation")?;
    let since = presumed_utc(&request.get("since")?)?;
    let schema: Bound<'py, PyAny> = native_field(&relation, "schema")?;
    let schema: Bound<'py, PyAny> = if schema.is_none() {
        PyString::new(py, "").into_any()
    } else {
        schema
    };
    let events: Bound<'py, PyTuple> = read_events(
        &access(&request)?,
        &layout(COLUMN_KIND)?,
        (native_field(&relation, "database")?, schema),
        None,
    )?;
    let keys: Vec<RecordedRenameKey> = events
        .iter()
        .map(|event| {
            Ok(RecordedRenameKey {
                order: event_order(&event)?,
                relation: event_relation(&native_field(&event, "relation")?)?,
                records_event: native_field(&native_field(&event, "decision")?, "records_event")?
                    .is_truthy()?,
            })
        })
        .collect::<PyResult<_>>()?;
    let target = event_relation(&relation)?;
    let selected: Vec<Bound<'py, PyAny>> = recorded_renames(&keys, &target, &since)
        .into_iter()
        .map(|index| events.get_item(index))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, selected)
}

/// Create the state table from adapter DDL when given, then append the event unless recorded.
#[pyfunction(name = "write_migration_state_event")]
#[pyo3(signature = (**request))]
pub(crate) fn write_state_event(request: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let request = Request::new("write_migration_state_event", request)?;
    let create_table_sql: Option<String> = request.get("create_table_sql")?.extract()?;
    let attempts: i64 = match request.optional("attempts")? {
        Some(attempts) => attempts.extract()?,
        None => MIGRATION_WRITE_ATTEMPTS,
    };
    append_event(
        &access(&request)?,
        (
            &request.get("kind")?.extract::<String>()?,
            &request.get("event")?,
        ),
        create_table_sql.as_deref(),
        attempts,
    )
}
