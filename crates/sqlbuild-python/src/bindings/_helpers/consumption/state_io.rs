//! Consumption table DDL, recent-execution reads and chunked appends through adapter callbacks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyInt, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_state::consumption::constants::{
    CONSUMPTION_COLUMN_TYPES, INTEGER_STATE_VALUE_TYPE, NULLABLE_COLUMNS, RECENT_EXECUTIONS,
    RECORDED_AT_COLUMN,
};
use sqlbuild_state::consumption::main::prune_executions_sql::prune_executions_sql;
use sqlbuild_state::consumption::main::recent_executions_sql::recent_executions_sql;
use sqlbuild_state::consumption::models::{
    ConsumedInput, ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey,
};
use sqlbuild_state::freshness::constants::STRING_STATE_VALUE_TYPE;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::freshness::support::presumed_utc_datetime;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::state_rows::state_tables::state_table_create_sql;
use crate::bindings::_helpers::state_rows::timestamps::date_time_typed;

/// Rows per `INSERT`; SQL Server accepts at most 1,000 rows in one `VALUES` list.
const ROWS_PER_STATEMENT: usize = 500;

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

    fn keywords<'py>(&self, py: Python<'py>) -> Vec<(&'static str, Bound<'py, PyAny>)> {
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

/// The adapter's qualified name of the consumption table in `(database, schema)`.
fn qualified_name(
    render_qualified_name: &Bound<'_, PyAny>,
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Option<String>> {
    let py: Python<'_> = render_qualified_name.py();
    let name: Bound<'_, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", text(py, CONSUMPTION_TABLE_NAME)),
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
    qualified_name(
        &native_field(adapter, "render_qualified_name")?,
        (&database, &text(py, &location.schema)),
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
    let py: Python<'_> = database.py();
    let Some(table) = qualified_name(render_qualified_name, (database, schema))? else {
        return Ok(String::new());
    };
    let columns: Bound<'_, PyAny> = PyTuple::new(
        py,
        CONSUMPTION_COLUMN_TYPES.iter().map(|(column, _)| *column),
    )?
    .into_any();
    let types: Bound<'_, PyDict> = PyDict::new(py);
    for (column, declared) in CONSUMPTION_COLUMN_TYPES {
        types.set_item(column, declared)?;
    }
    let required: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .filter(|column| !NULLABLE_COLUMNS.contains(column))
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
    Ok(qualified_name(render_qualified_name, (database, schema))?
        .map(|table| prune_executions_sql(&table, retain_versions))
        .unwrap_or_default())
}

fn execute<'py>(
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

/// Create the table and its indexes in `location` when missing.
pub(crate) fn ensure_table(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    let keywords = location.keywords(py);
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
    result
        .call_method0("fetchall")?
        .try_iter()?
        .map(|row| read_row(&row?))
        .collect()
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

fn required_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(optional_text(value)?.unwrap_or_default())
}

/// Microseconds since the epoch of a warehouse timestamp, naive values read as UTC.
fn epoch_micros(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    let py: Python<'_> = value.py();
    let moment: Bound<'_, PyAny> = if date_time_typed(value)? {
        value.clone()
    } else {
        imported(py, "datetime", "datetime")?.call_method1("fromisoformat", (value.str()?,))?
    };
    let utc: Bound<'_, PyAny> = presumed_utc_datetime(&moment)?;
    let epoch: Bound<'_, PyAny> = imported(py, "datetime", "datetime")?
        .call_method1("fromtimestamp", (0, imported(py, "datetime", "UTC")?))?;
    let micros: Bound<'_, PyAny> = imported(py, "datetime", "timedelta")?.call1((0, 0, 1))?;
    utc.call_method1("__sub__", (epoch,))?
        .call_method1("__floordiv__", (micros,))?
        .extract()
}

fn read_row(row: &Bound<'_, PyAny>) -> PyResult<ConsumptionRow> {
    let cell = |index: usize| row.get_item(index);
    let input_kind: Option<String> = optional_text(&cell(11)?)?;
    let input: Option<ConsumedInput> = match input_kind {
        Some(kind) => Some(ConsumedInput {
            kind,
            name: required_text(&cell(12)?)?,
            version: required_text(&cell(13)?)?,
        }),
        None => None,
    };
    let count: Bound<'_, PyAny> = cell(10)?;
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
            event_id: required_text(&cell(0)?)?,
            execution_id: required_text(&cell(1)?)?,
            phase: required_text(&cell(2)?)?,
            run_id: required_text(&cell(3)?)?,
            node: NodeKey {
                kind: required_text(&cell(4)?)?,
                name: required_text(&cell(5)?)?,
            },
            relation: RelationKey {
                database: optional_text(&cell(6)?)?,
                schema: required_text(&cell(7)?)?,
                name: required_text(&cell(8)?)?,
            },
            version_hash: required_text(&cell(9)?)?,
            input_count,
            input,
        },
        recorded_at: epoch_micros(&cell(14)?)?,
    })
}

fn literal(py: Python<'_>, value: Option<&str>) -> PyResult<String> {
    let value: Bound<'_, PyAny> =
        value.map_or_else(|| py.None().into_bound(py), |value| text(py, value));
    render_state_sql_literal(py, &value, STRING_STATE_VALUE_TYPE, None)
}

fn row_cells(py: Python<'_>, row: &ConsumptionAppend, recorded_at: &str) -> PyResult<Vec<String>> {
    let input = row.input.as_ref();
    Ok(vec![
        literal(py, Some(&row.event_id))?,
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

/// Append `rows` to `location`'s table in statements of whole executions where they fit.
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
    let recorded_at: String = adapter
        .call_method0("render_current_utc_timestamp")?
        .extract()?;
    let columns: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    debug_assert_eq!(columns.last(), Some(&RECORDED_AT_COLUMN));
    for chunk in execution_chunks(rows) {
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

/// Contiguous rows in statements of whole executions, splitting only an execution that alone overflows.
fn execution_chunks(rows: &[ConsumptionAppend]) -> Vec<&[ConsumptionAppend]> {
    let mut chunks: Vec<&[ConsumptionAppend]> = Vec::new();
    let (mut start, mut index): (usize, usize) = (0, 0);
    while index < rows.len() {
        let end: usize = index
            + rows[index..]
                .iter()
                .take_while(|row| row.execution_id == rows[index].execution_id)
                .count();
        if end - start > ROWS_PER_STATEMENT && index > start {
            chunks.push(&rows[start..index]);
            start = index;
        }
        while end - start > ROWS_PER_STATEMENT {
            chunks.push(&rows[start..start + ROWS_PER_STATEMENT]);
            start += ROWS_PER_STATEMENT;
        }
        index = end;
    }
    if start < rows.len() {
        chunks.push(&rows[start..]);
    }
    chunks
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_consumption_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_consumption_prune_sql, module)?)
}
