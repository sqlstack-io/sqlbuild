//! The consumption table through the adapter, and the connection and callbacks a recording build holds.

use chrono::{DateTime, NaiveDateTime, Utc};
use std::sync::Mutex;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyInt, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_state::consumption::constants::{
    CONSUMPTION_COLUMN_TYPES, INTEGER_STATE_VALUE_TYPE, NULLABLE_COLUMNS, RECENT_EXECUTIONS,
};
use sqlbuild_state::consumption::main::executions_by_id_sql::executions_by_id_sql;
use sqlbuild_state::consumption::main::prune_executions_sql::prune_executions_sql;
use sqlbuild_state::consumption::main::recent_executions_sql::recent_executions_sql;
use sqlbuild_state::consumption::models::{
    ConsumedInput, ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey,
};
use sqlbuild_state::freshness::constants::STRING_STATE_VALUE_TYPE;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::native_host::synced_copy::SyncedCopyAdapter;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::state_rows::state_tables::state_table_create_sql;

/// Rows per `INSERT`; SQL Server accepts at most 1,000 rows in one `VALUES` list.
pub(crate) const ROWS_PER_STATEMENT: usize = 500;
/// Execution IDs per `IN` list when reading executions back by ID.
const EXECUTION_ID_BATCH: usize = 500;
/// Warehouse timestamp texts with a UTC offset, as drivers render aware values.
const OFFSET_TIME_FORMATS: [&str; 2] = ["%Y-%m-%dT%H:%M:%S%.f%:z", "%Y-%m-%d %H:%M:%S%.f%:z"];
/// Warehouse timestamp texts without an offset, which the consumption table stores as UTC.
const NAIVE_TIME_FORMATS: [&str; 2] = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"];

/// One destination schema of the consumption table.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Location {
    pub(crate) database: Option<String>,
    pub(crate) schema: String,
}

impl Location {
    pub(crate) fn of(relation: &RelationKey) -> Self {
        Self {
            database: relation.database.clone(),
            schema: relation.schema.clone(),
        }
    }

    pub(crate) fn keywords<'py>(&self, py: Python<'py>) -> Vec<(&'static str, Bound<'py, PyAny>)> {
        vec![
            (
                "database",
                self.database
                    .as_deref()
                    .map_or_else(|| py.None().into_bound(py), |database| text(py, database)),
            ),
            ("schema", text(py, &self.schema)),
        ]
    }
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// The adapter's qualified name of a state table in `(database, schema)`, if it has one.
pub(crate) fn state_table_name(
    render_qualified_name: &Bound<'_, PyAny>,
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    table: &str,
) -> PyResult<Option<String>> {
    let py: Python<'_> = render_qualified_name.py();
    let name: Bound<'_, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", text(py, table)),
        ],
    )?;
    if name.is_none() {
        return Ok(None);
    }
    Ok(Some(name.extract()?))
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
fn build_consumption_create_table_sql(
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

/// Create-if-missing DDL for a state table with fixed columns, typed by the adapter.
pub(crate) fn fixed_state_table_sql(
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (render_qualified_name, render_framework_type): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (table_name, column_types, nullable): (&str, &[(&str, &str)], &[&str]),
) -> PyResult<String> {
    let py: Python<'_> = database.py();
    let Some(table) = state_table_name(render_qualified_name, (database, schema), table_name)?
    else {
        return Ok(String::new());
    };
    let columns: Bound<'_, PyAny> =
        PyTuple::new(py, column_types.iter().map(|(column, _)| *column))?.into_any();
    let types: Bound<'_, PyDict> = PyDict::new(py);
    for (column, declared) in column_types {
        types.set_item(column, declared)?;
    }
    let required: Vec<&str> = column_types
        .iter()
        .map(|(column, _)| *column)
        .filter(|column| !nullable.contains(column))
        .collect();
    state_table_create_sql(
        &table,
        (
            &columns,
            &types.into_any(),
            &PyFrozenSet::new(py, &required)?.into_any(),
        ),
        render_framework_type,
    )
}

/// SQL deleting whole consumption executions beyond the newest `retain_versions` per relation.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
fn build_consumption_prune_sql(
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

pub(crate) fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
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

pub(crate) fn literal(py: Python<'_>, value: Option<&str>) -> PyResult<String> {
    let value: Bound<'_, PyAny> =
        value.map_or_else(|| py.None().into_bound(py), |value| text(py, value));
    render_state_sql_literal(py, &value, STRING_STATE_VALUE_TYPE, None)
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
        render_state_sql_literal(
            py,
            &PyInt::new(py, row.input_count).into_any(),
            INTEGER_STATE_VALUE_TYPE,
            None,
        )?,
        literal(py, input.map(|input| input.kind.as_str()))?,
        literal(py, input.map(|input| input.name.as_str()))?,
        literal(py, input.map(|input| input.version.as_str()))?,
        recorded_at.to_owned(),
    ])
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
        if let Ok(copy) = self.adapter.bind(py).downcast::<SyncedCopyAdapter>() {
            return Ok(copy.get().consumption(location));
        }
        read_recent(self.adapter.bind(py), &self.connected(py)?, location)
    }

    pub(crate) fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        if let Ok(copy) = self.adapter.bind(py).downcast::<SyncedCopyAdapter>() {
            let mut rows: Vec<ConsumptionRow> = copy.get().consumption(location);
            rows.retain(|row| execution_ids.contains(&row.row.execution_id.as_str()));
            return Ok(rows);
        }
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

/// The build's own node-completion callback, which the recorder forwards every result to.
pub(crate) struct NodeCompleteHook(Py<PyAny>);

impl NodeCompleteHook {
    pub(crate) fn new(callback: &Bound<'_, PyAny>) -> Self {
        Self(callback.clone().unbind())
    }

    pub(crate) fn forward(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        self.0.bind(result.py()).call1((result,))?;
        Ok(())
    }
}

/// The recorder's `on_node_complete` and `before_node_write` methods, as build callbacks.
pub(crate) fn recorder_callbacks<'py>(
    recorder: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    Ok((
        recorder.getattr("on_node_complete")?,
        recorder.getattr("before_node_write")?,
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_consumption_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_consumption_prune_sql, module)?)
}
