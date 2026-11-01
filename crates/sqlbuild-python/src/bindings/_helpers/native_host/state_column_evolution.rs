//! Warehouse catalog and nullable-column DDL adapter edge for persisted event identity.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field;

/// Missing callbacks retain the existing renderer-only API for already-current/new schemas.
pub(crate) fn ensure_event_id_column<'py>(
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (table, qualified): (&str, &str),
    (read_columns, render_add, render_type): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<()> {
    if read_columns.is_none() && render_add.is_none() {
        return Ok(());
    }
    if read_columns.is_none() || render_add.is_none() {
        return Err(PyValueError::new_err(
            "state column evolution requires both read_columns and render_add_columns",
        ));
    }
    let py = connection.py();
    let scope = [
        ("connection", connection.clone()),
        ("database", database.clone()),
        ("schema", schema.clone()),
        ("name", PyString::new(py, table).into_any()),
    ];
    if event_id_present(read_columns, &scope)? {
        return Ok(());
    }
    let string_type = render_type.call1((field(
        &host::entry(py, AdapterHost::FrameworkType)?,
        "STRING",
    )?,))?;
    let column = call_with(
        &host::entry(py, AdapterHost::ColumnInfo)?,
        &[
            ("name", PyString::new(py, "event_id").into_any()),
            ("type", string_type),
        ],
    )?;
    let statements = call_with(
        render_add,
        &[
            ("destination", PyString::new(py, qualified).into_any()),
            ("columns", PyTuple::new(py, [column])?.into_any()),
        ],
    )?;
    for statement in statements.try_iter()? {
        if let Err(error) = call_with(
            execute,
            &[("connection", connection.clone()), ("sql", statement?)],
        ) && !event_id_present(read_columns, &scope).unwrap_or(false)
        {
            return Err(error);
        }
    }
    Ok(())
}

/// Concurrent additions are harmless only once the authoritative catalog confirms the column.
fn event_id_present(
    read_columns: &Bound<'_, PyAny>,
    scope: &[(&str, Bound<'_, PyAny>)],
) -> PyResult<bool> {
    for column in call_with(read_columns, scope)?.try_iter()? {
        if field(&column?, "name")?
            .extract::<String>()?
            .eq_ignore_ascii_case("event_id")
        {
            return Ok(true);
        }
    }
    Ok(false)
}
