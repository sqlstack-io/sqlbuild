//! The consumption table through the adapter: its reads and writes.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_state::consumption::constants::{CONSUMPTION_COLUMN_TYPES, RECENT_EXECUTIONS};
use sqlbuild_state::consumption::main::executions_by_id_sql::executions_by_id_sql;
use sqlbuild_state::consumption::main::recent_executions_sql::recent_executions_sql;
use sqlbuild_state::consumption::models::{ConsumptionAppend, ConsumptionRow};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::native_host::_helpers::consumption_state::{EXECUTION_ID_BATCH, read_row, row_cells};
use crate::native_host::constants::ROWS_PER_STATEMENT;
use crate::native_host::main::state_table_name::state_table_name;
use crate::native_host::models::Location;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

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
pub(crate) fn ensure_table(
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
pub(crate) fn table_exists(
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

/// The consumption table of `location` when `wanted` and the warehouse lists it.
fn readable_table(
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    location: &Location,
    wanted: bool,
) -> PyResult<Option<String>> {
    let Some(table) = table_of(adapter, location)? else {
        return Ok(None);
    };
    if !wanted || !table_exists(adapter, connection, location)? {
        return Ok(None);
    }
    Ok(Some(table))
}

/// The consumption rows `sql` reads.
fn fetched_rows(
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    sql: &str,
) -> PyResult<Vec<ConsumptionRow>> {
    let result: Bound<'_, PyAny> = execute(adapter, connection, sql)?;
    call_with(&native_field(&result, "fetchall")?, &[])?
        .try_iter()?
        .map(|row| read_row(&row?))
        .collect()
}

/// Rows of the newest executions per relation in `location`; none when the table is missing.
pub(crate) fn read_recent(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<Vec<ConsumptionRow>> {
    match readable_table((adapter, connection), location, true)? {
        Some(table) => fetched_rows(
            (adapter, connection),
            &recent_executions_sql(&table, RECENT_EXECUTIONS),
        ),
        None => Ok(Vec::new()),
    }
}

/// Rows of the named executions in `location`, read in batches; none when the table is missing.
pub(crate) fn read_executions(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    execution_ids: &[&str],
) -> PyResult<Vec<ConsumptionRow>> {
    let Some(table) = readable_table((adapter, connection), location, !execution_ids.is_empty())?
    else {
        return Ok(Vec::new());
    };
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for batch in execution_ids.chunks(EXECUTION_ID_BATCH) {
        rows.extend(fetched_rows(
            (adapter, connection),
            &executions_by_id_sql(&table, batch),
        )?);
    }
    Ok(rows)
}

/// Append `rows` to `location`'s table, in statements no longer than every warehouse accepts.
pub(crate) fn append(
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
