//! The consumption table through the adapter: the connection a recording build holds and its reads and writes.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_state::consumption::constants::{CONSUMPTION_COLUMN_TYPES, RECENT_EXECUTIONS};
use sqlbuild_state::consumption::main::executions_by_id_sql::executions_by_id_sql;
use sqlbuild_state::consumption::main::recent_executions_sql::recent_executions_sql;
use sqlbuild_state::consumption::models::{ConsumptionAppend, ConsumptionRow};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::consumption_state::{
    ConsumptionReads, EXECUTION_ID_BATCH, Location, ROWS_PER_STATEMENT, read_row, row_cells,
    state_table_name,
};
use crate::bindings::_helpers::records::fields::field as native_field;

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn table_of(adapter: &Bound<'_, PyAny>, location: &Location) -> PyResult<Option<String>> {
    let py: Python<'_> = adapter.py();
    let database: Bound<'_, PyAny> = location
        .database
        .as_deref()
        .map_or_else(|| py.None().into_bound(py), |database| text(py, database));
    state_table_name(
        &native_field(adapter, "render_qualified_name")?,
        (&database, &text(py, &location.schema)),
        CONSUMPTION_TABLE_NAME,
    )
}

pub(crate) fn execute<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    sql: &str,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &native_field(adapter, "execute")?,
        &[
            ("connection", connection.clone()),
            ("sql", text(adapter.py(), sql)),
        ],
    )
}

/// Create the schema, table and indexes in `location`; `started` rows precede schema preparation.
fn ensure_table(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    let keywords = location.keywords(py);
    for statement in
        call_with(&native_field(adapter, "render_create_schema")?, &keywords)?.try_iter()?
    {
        execute(adapter, connection, &statement?.extract::<String>()?)?;
    }
    let create: String = call_with(
        &native_field(adapter, "render_create_consumption_table_sql")?,
        &keywords,
    )?
    .extract()?;
    if !create.is_empty() {
        execute(adapter, connection, &create)?;
    }
    for index in call_with(
        &native_field(adapter, "render_create_consumption_index_sqls")?,
        &keywords,
    )?
    .try_iter()?
    {
        execute(adapter, connection, &index?.extract::<String>()?)?;
    }
    Ok(())
}

/// Whether the warehouse catalog lists the consumption table in `location`.
fn table_exists(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<bool> {
    let py: Python<'_> = adapter.py();
    let mut keywords = location.keywords(py);
    keywords.push(("connection", connection.clone()));
    keywords.push(("name", text(py, CONSUMPTION_TABLE_NAME)));
    call_with(&native_field(adapter, "get_columns")?, &keywords)?.is_truthy()
}

/// Rows of the newest executions per relation in `location`; none when the table is missing.
fn read_recent(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<Vec<ConsumptionRow>> {
    let Some(table) = table_of(adapter, location)? else {
        return Ok(Vec::new());
    };
    if !table_exists(adapter, connection, location)? {
        return Ok(Vec::new());
    }
    let result: Bound<'_, PyAny> = execute(
        adapter,
        connection,
        &recent_executions_sql(&table, RECENT_EXECUTIONS),
    )?;
    call_with(&native_field(&result, "fetchall")?, &[])?
        .try_iter()?
        .map(|row| read_row(&row?))
        .collect()
}

/// Rows of the named executions in `location`, read in batches; none when the table is missing.
fn read_executions(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    execution_ids: &[&str],
) -> PyResult<Vec<ConsumptionRow>> {
    let Some(table) = table_of(adapter, location)? else {
        return Ok(Vec::new());
    };
    if execution_ids.is_empty() || !table_exists(adapter, connection, location)? {
        return Ok(Vec::new());
    }
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for batch in execution_ids.chunks(EXECUTION_ID_BATCH) {
        let result: Bound<'_, PyAny> =
            execute(adapter, connection, &executions_by_id_sql(&table, batch))?;
        for row in call_with(&native_field(&result, "fetchall")?, &[])?.try_iter()? {
            rows.push(read_row(&row?)?);
        }
    }
    Ok(rows)
}

/// Append `rows` to `location`'s table, in statements no longer than every warehouse accepts.
fn append(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    rows: &[ConsumptionAppend],
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    let Some(table) = table_of(adapter, location)? else {
        return Ok(());
    };
    let recorded_at: String =
        call_with(&native_field(adapter, "render_current_utc_timestamp")?, &[])?.extract()?;
    let columns: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    for chunk in rows.chunks(ROWS_PER_STATEMENT) {
        let cells: Vec<Vec<String>> = chunk
            .iter()
            .map(|row| row_cells(py, row, &recorded_at))
            .collect::<PyResult<_>>()?;
        execute(
            adapter,
            connection,
            &values_insert_sql(&table, &columns, &cells),
        )?;
    }
    Ok(())
}

/// One build's consumption I/O, connected only per phase so loaders can lock file warehouses.
pub(crate) struct ConsumptionConnection {
    adapter: Py<PyAny>,
    connection_config: Py<PyAny>,
    connection: Mutex<Option<Py<PyAny>>>,
}

impl ConsumptionConnection {
    pub(crate) fn new(adapter: &Bound<'_, PyAny>, connection_config: &Bound<'_, PyAny>) -> Self {
        Self {
            adapter: adapter.clone().unbind(),
            connection_config: connection_config.clone().unbind(),
            connection: Mutex::new(None),
        }
    }

    pub(crate) fn connected<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut open = self
            .connection
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption connection lock poisoned"))?;
        if let Some(connection) = open.as_ref() {
            return Ok(connection.bind(py).clone());
        }
        let connection: Bound<'py, PyAny> = native_field(self.adapter.bind(py), "connect")?
            .call1((self.connection_config.bind(py),))?;
        *open = Some(connection.clone().unbind());
        Ok(connection)
    }

    pub(crate) fn ensure_table(&self, py: Python<'_>, location: &Location) -> PyResult<()> {
        ensure_table(self.adapter.bind(py), &self.connected(py)?, location)
    }

    pub(crate) fn read_recent(
        &self,
        py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>> {
        read_recent(self.adapter.bind(py), &self.connected(py)?, location)
    }

    pub(crate) fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        read_executions(
            self.adapter.bind(py),
            &self.connected(py)?,
            location,
            execution_ids,
        )
    }

    pub(crate) fn append(
        &self,
        py: Python<'_>,
        location: &Location,
        rows: &[ConsumptionAppend],
    ) -> PyResult<()> {
        append(self.adapter.bind(py), &self.connected(py)?, location, rows)
    }

    /// Close the connection if one is open; the next operation connects again.
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
        let open: Option<Py<PyAny>> = self
            .connection
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption connection lock poisoned"))?
            .take();
        if let Some(connection) = open {
            native_field(self.adapter.bind(py), "close")?.call1((connection.bind(py),))?;
        }
        Ok(())
    }
}

impl ConsumptionReads for ConsumptionConnection {
    fn read_recent(&self, py: Python<'_>, location: &Location) -> PyResult<Vec<ConsumptionRow>> {
        ConsumptionConnection::read_recent(self, py, location)
    }

    fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        ConsumptionConnection::read_executions(self, py, location, execution_ids)
    }
}
