//! The consumption table through the adapter, and the connection and callbacks a recording build holds.

use crate::native_host::constants::ROWS_PER_STATEMENT;
use crate::native_host::main::execute::execute;
use crate::native_host::main::fixed_state_table_sql::fixed_state_table_sql;
use crate::native_host::main::integer_literal::integer_literal;
use crate::native_host::main::literal::literal;
use crate::native_host::main::optional_text::optional_text;
use crate::native_host::main::state_table_name::state_table_name;
use crate::native_host::models::Location;
use chrono::{DateTime, NaiveDateTime, Utc};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyInt, PyString};
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_state::consumption::constants::{
    CONSUMPTION_COLUMN_TYPES, NULLABLE_COLUMNS, RECENT_EXECUTIONS,
};
use sqlbuild_state::consumption::main::executions_by_id_sql::executions_by_id_sql;
use sqlbuild_state::consumption::main::prune_executions_sql::prune_executions_sql;
use sqlbuild_state::consumption::main::recent_executions_sql::recent_executions_sql;
use sqlbuild_state::consumption::models::{
    ConsumedInput, ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey,
};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::contracts::main::render_state_sql_literal::render_state_sql_literal;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Execution IDs per `IN` list when reading executions back by ID.
const EXECUTION_ID_BATCH: usize = 500;
/// Warehouse timestamp texts with a UTC offset, as drivers render aware values.
const OFFSET_TIME_FORMATS: [&str; 2] = ["%Y-%m-%dT%H:%M:%S%.f%:z", "%Y-%m-%d %H:%M:%S%.f%:z"];
/// Warehouse timestamp texts without an offset, which the consumption table stores as UTC.
const NAIVE_TIME_FORMATS: [&str; 2] = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"];

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
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

/// Create-if-missing DDL for the consumption table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
pub(crate) fn build_consumption_create_table_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    fixed_state_table_sql(
        (database, schema),
        (render_qualified_name, render_framework_type),
        (
            CONSUMPTION_TABLE_NAME,
            &CONSUMPTION_COLUMN_TYPES,
            &NULLABLE_COLUMNS,
        ),
    )
}

/// SQL deleting whole consumption executions beyond the newest `retain_versions` per relation.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
pub(crate) fn build_consumption_prune_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    retain_versions: i64,
) -> PyResult<String> {
    Ok(state_table_name(
        render_qualified_name,
        (database, schema),
        CONSUMPTION_TABLE_NAME,
    )?
    .map(|table| prune_executions_sql(&table, retain_versions))
    .unwrap_or_default())
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
pub(crate) fn read_recent(
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
pub(crate) fn read_executions(
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

fn required_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(optional_text(value)?.unwrap_or_default())
}

/// Microseconds since the epoch of a warehouse timestamp's text, naive values read as UTC.
fn epoch_micros(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    let text: String = value.str()?.extract()?;
    let text: &str = text.trim();
    for format in OFFSET_TIME_FORMATS {
        if let Ok(moment) = DateTime::parse_from_str(text, format) {
            return Ok(moment.with_timezone(&Utc).timestamp_micros());
        }
    }
    for format in NAIVE_TIME_FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            return Ok(naive.and_utc().timestamp_micros());
        }
    }
    Err(PyValueError::new_err(format!(
        "unreadable consumption time {text:?}"
    )))
}

fn read_row(row: &Bound<'_, PyAny>) -> PyResult<ConsumptionRow> {
    let cell = |index: usize| row.get_item(index);
    let input_kind: Option<String> = optional_text(&cell(10)?)?;
    let input: Option<ConsumedInput> = match input_kind {
        Some(kind) => Some(ConsumedInput {
            kind,
            name: required_text(&cell(11)?)?,
            version: required_text(&cell(12)?)?,
        }),
        None => None,
    };
    let count: Bound<'_, PyAny> = cell(9)?;
    let input_count: i64 = if count.is_instance_of::<PyInt>() {
        count.extract()?
    } else {
        count
            .str()?
            .extract::<String>()?
            .trim()
            .parse()
            .unwrap_or(-1)
    };
    Ok(ConsumptionRow {
        row: ConsumptionAppend {
            execution_id: required_text(&cell(0)?)?,
            phase: required_text(&cell(1)?)?,
            run_id: required_text(&cell(2)?)?,
            node: NodeKey {
                kind: required_text(&cell(3)?)?,
                name: required_text(&cell(4)?)?,
            },
            relation: RelationKey {
                database: optional_text(&cell(5)?)?,
                schema: required_text(&cell(6)?)?,
                name: required_text(&cell(7)?)?,
            },
            version_hash: required_text(&cell(8)?)?,
            input_count,
            input,
        },
        recorded_at: epoch_micros(&cell(13)?)?,
    })
}

/// A nullable state value as a SQL literal of `declared_type`.
pub(crate) fn state_literal(
    py: Python<'_>,
    value: Option<Bound<'_, PyAny>>,
    declared_type: &str,
) -> PyResult<String> {
    let value: Bound<'_, PyAny> = value.unwrap_or_else(|| py.None().into_bound(py));
    render_state_sql_literal(py, &value, declared_type, None)
}

fn row_cells(py: Python<'_>, row: &ConsumptionAppend, recorded_at: &str) -> PyResult<Vec<String>> {
    let input = row.input.as_ref();
    Ok(vec![
        literal(py, Some(&row.execution_id))?,
        literal(py, Some(&row.phase))?,
        literal(py, Some(&row.run_id))?,
        literal(py, Some(&row.node.kind))?,
        literal(py, Some(&row.node.name))?,
        literal(py, row.relation.database.as_deref())?,
        literal(py, Some(&row.relation.schema))?,
        literal(py, Some(&row.relation.name))?,
        literal(py, Some(&row.version_hash))?,
        integer_literal(py, Some(row.input_count))?,
        literal(py, input.map(|input| input.kind.as_str()))?,
        literal(py, input.map(|input| input.name.as_str()))?,
        literal(py, input.map(|input| input.version.as_str()))?,
        recorded_at.to_owned(),
    ])
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
