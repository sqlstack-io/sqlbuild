//! Source freshness state table DDL, latest-row reads and appends through adapter callbacks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME;
use sqlbuild_state::freshness::constants::{
    REQUIRED_STATE_COLUMNS, SOURCE_FRESHNESS_COLUMN_TYPES, SOURCE_FRESHNESS_EVENT_ID,
    STRING_STATE_VALUE_TYPE,
};
use sqlbuild_state::freshness::main::read_latest_sql::read_latest_sql;
use sqlbuild_state::freshness::main::relevant_sources_sql::relevant_sources_sql;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::cursor::values::DATETIME_MODULE;
use crate::bindings::_helpers::freshness::support::{
    INPUT_ERROR, exception_text, freshness_error, optional_str, presumed_utc_datetime, text,
};
use crate::bindings::_helpers::native_host::state_column_evolution::ensure_event_id_column;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::state_rows::state_tables::state_table_create_sql;
use crate::bindings::_helpers::state_rows::timestamps::date_time_typed;

const MISSING_SCHEMA_MESSAGE: &str = "source freshness table requires a target schema";

/// The qualified state table name the adapter renders for one target schema.
pub(crate) fn qualified_table_name(
    render_qualified_name: &Bound<'_, PyAny>,
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<String> {
    let py: Python<'_> = render_qualified_name.py();
    let name: Bound<'_, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", text(py, SOURCE_FRESHNESS_TABLE_NAME)),
        ],
    )?;
    if name.is_none() {
        return Err(freshness_error(
            py,
            (INPUT_ERROR, MISSING_SCHEMA_MESSAGE),
            None,
        ));
    }
    name.extract()
}

/// SQL selecting the latest source freshness row per identity.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name))]
fn build_source_freshness_read_latest_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
) -> PyResult<String> {
    Ok(read_latest_sql(&qualified_table_name(
        render_qualified_name,
        (database, schema),
    )?))
}

/// Create-if-missing DDL for the source freshness state table.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
fn build_source_freshness_create_table_sql<'py>(
    database: &Bound<'py, PyAny>,
    schema: &Bound<'py, PyAny>,
    render_qualified_name: &Bound<'py, PyAny>,
    render_framework_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    create_table_sql(
        (database, schema),
        render_qualified_name,
        render_framework_type,
    )
}

fn create_table_sql<'py>(
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    render_qualified_name: &Bound<'py, PyAny>,
    render_framework_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = database.py();
    let qualified_name: String = qualified_table_name(render_qualified_name, (database, schema))?;
    let columns: Bound<'py, PyAny> = PyTuple::new(
        py,
        SOURCE_FRESHNESS_COLUMN_TYPES
            .iter()
            .map(|(column, _)| *column)
            .chain([SOURCE_FRESHNESS_EVENT_ID])
            .collect::<Vec<_>>(),
    )?
    .into_any();
    let column_types: Bound<'py, PyDict> = PyDict::new(py);
    for (column, declared) in SOURCE_FRESHNESS_COLUMN_TYPES {
        column_types.set_item(column, declared)?;
    }
    column_types.set_item(SOURCE_FRESHNESS_EVENT_ID, STRING_STATE_VALUE_TYPE)?;
    let column_types: Bound<'py, PyAny> = column_types.into_any();
    let required: Bound<'py, PyAny> = PyFrozenSet::new(py, REQUIRED_STATE_COLUMNS)?.into_any();
    let sql: String = state_table_create_sql(
        &qualified_name,
        (&columns, &column_types, &required),
        render_framework_type,
    )?;
    Ok(text(py, &sql))
}

/// The callbacks and location one latest-state read uses.
pub(crate) struct StateRead<'py> {
    pub(crate) connection: Bound<'py, PyAny>,
    pub(crate) execute: Bound<'py, PyAny>,
    pub(crate) render_qualified_name: Bound<'py, PyAny>,
    pub(crate) render_read_latest_sql: Bound<'py, PyAny>,
}

fn empty_set<'py>(schema: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = schema.py();
    new_record_with(
        py,
        RecordId::SourceFreshnessSet,
        &[
            ("schema", schema.clone()),
            ("records", PyDict::new(py).into_any()),
        ],
    )
}

/// Latest records of one schema, trusting the caller-resolved table existence.
pub(crate) fn read_latest<'py>(
    read: &StateRead<'py>,
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    table_exists: bool,
    source_names: Option<&[String]>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = schema.py();
    let qualified_name: String =
        qualified_table_name(&read.render_qualified_name, (database, schema))?;
    if !table_exists {
        return empty_set(schema);
    }
    let mut read_sql: String = call_with(
        &read.render_read_latest_sql,
        &[("database", database.clone()), ("schema", schema.clone())],
    )?
    .extract()?;
    if let Some(source_names) = source_names {
        if source_names.is_empty() {
            return empty_set(schema);
        }
        read_sql = relevant_sources_sql(&read_sql, source_names);
    }
    let result: Bound<'py, PyAny> = match call_with(
        &read.execute,
        &[
            ("connection", read.connection.clone()),
            ("sql", text(py, &read_sql)),
        ],
    ) {
        Ok(result) => result,
        Err(error) => {
            if exception_text(py, &error)?.is_none() {
                return Err(error);
            }
            return Err(freshness_error(
                py,
                (
                    INPUT_ERROR,
                    &format!(
                        "Unable to read source freshness records from {qualified_name}. This can \
happen after upgrading from an older sqlbuild version; delete or rebuild the SQLBuild source \
freshness table to regenerate source freshness state."
                    ),
                ),
                Some(error),
            ));
        }
    };
    let records: Bound<'py, PyDict> = PyDict::new(py);
    for row in result.call_method0("fetchall")?.try_iter()? {
        let record: Bound<'py, PyAny> = row_record(&row?)?;
        records.set_item(native_field(&record, "identity")?, record)?;
    }
    new_record_with(
        py,
        RecordId::SourceFreshnessSet,
        &[("schema", schema.clone()), ("records", records.into_any())],
    )
}

pub(crate) fn row_record<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = row.py();
    let raw_observed_at: Bound<'py, PyAny> = row.get_item(9)?;
    let observed_at: Bound<'py, PyAny> = if date_time_typed(&raw_observed_at)? {
        raw_observed_at
    } else {
        imported(py, DATETIME_MODULE, "datetime")?
            .call_method1("fromisoformat", (raw_observed_at.str()?,))?
    };
    let observed_at: Bound<'py, PyAny> = presumed_utc_datetime(&observed_at)?;
    let field = |index: usize| -> PyResult<Bound<'py, PyAny>> {
        Ok(row.get_item(index)?.str()?.into_any())
    };
    let optional =
        |index: usize| -> PyResult<Bound<'py, PyAny>> { optional_str(&row.get_item(index)?) };
    new_record_with(
        py,
        RecordId::SourceFreshnessRecord,
        &[
            ("source_name", field(0)?),
            ("target_database", optional(1)?),
            ("target_schema", optional(2)?),
            ("target_name", optional(3)?),
            ("run_id", field(4)?),
            ("strategy", field(5)?),
            ("value_kind", field(6)?),
            ("data_version", optional(7)?),
            ("data_version_hash", field(8)?),
            ("observed_at", observed_at),
        ],
    )
}

/// Read latest source freshness rows, trusting the caller-resolved table existence.
#[pyfunction]
#[pyo3(signature = (**options))]
fn read_latest_source_freshness<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "read_latest_source_freshness",
        options,
        (
            &[
                "connection",
                "execute",
                "table_exists",
                "database",
                "schema",
                "render_qualified_name",
                "render_read_latest_sql",
            ],
            &["source_names"],
        ),
    )?;
    let read: StateRead<'py> = StateRead {
        connection: arguments.value("connection")?,
        execute: arguments.value("execute")?,
        render_qualified_name: arguments.value("render_qualified_name")?,
        render_read_latest_sql: arguments.value("render_read_latest_sql")?,
    };
    let source_names: Option<Vec<String>> = arguments.value("source_names")?.extract()?;
    read_latest(
        &read,
        (&arguments.value("database")?, &arguments.value("schema")?),
        arguments.value("table_exists")?.is_truthy()?,
        source_names.as_deref(),
    )
}

fn execute_sql(
    (execute, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    sql: Bound<'_, PyAny>,
) -> PyResult<()> {
    call_with(execute, &[("connection", connection.clone()), ("sql", sql)])?;
    Ok(())
}

/// Append source freshness rows, creating the table once if needed.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_source_freshness_records<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<()> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "write_source_freshness_records",
        options,
        (
            &[
                "connection",
                "execute",
                "database",
                "schema",
                "records",
                "renderers",
            ],
            &[],
        ),
    )?;
    let connection: &Bound<'py, PyAny> = &arguments.value("connection")?;
    let execute: &Bound<'py, PyAny> = &arguments.value("execute")?;
    let database: &Bound<'py, PyAny> = &arguments.value("database")?;
    let schema: &Bound<'py, PyAny> = &arguments.value("schema")?;
    let records: &Bound<'py, PyAny> = &arguments.value("records")?;
    let renderers: &Bound<'py, PyAny> = &arguments.value("renderers")?;
    if !records.is_truthy()? {
        return Ok(());
    }
    let location = || -> Vec<(&str, Bound<'py, PyAny>)> {
        vec![("database", database.clone()), ("schema", schema.clone())]
    };
    let render_create: Bound<'py, PyAny> = native_field(renderers, "render_create_table_sql")?;
    let create_sql: Bound<'py, PyAny> = if render_create.is_none() {
        create_table_sql(
            (database, schema),
            &native_field(renderers, "render_qualified_name")?,
            &native_field(renderers, "render_framework_type")?,
        )?
    } else {
        call_with(&render_create, &location())?
    };
    execute_sql((execute, connection), create_sql)?;
    let qualified = qualified_table_name(
        &native_field(renderers, "render_qualified_name")?,
        (database, schema),
    )?;
    ensure_event_id_column(
        (connection, execute),
        (database, schema),
        (SOURCE_FRESHNESS_TABLE_NAME, &qualified),
        (
            &native_field(renderers, "read_columns")?,
            &native_field(renderers, "render_add_columns")?,
            &native_field(renderers, "render_framework_type")?,
        ),
    )?;
    let render_indexes: Bound<'py, PyAny> = native_field(renderers, "render_create_index_sqls")?;
    if !render_indexes.is_none() {
        for index_sql in call_with(&render_indexes, &location())?.try_iter()? {
            execute_sql((execute, connection), index_sql?)?;
        }
    }
    let mut insert: Vec<(&str, Bound<'py, PyAny>)> = location();
    insert.push(("records", records.clone()));
    let insert_sql: Bound<'py, PyAny> = call_with(
        &native_field(renderers, "render_insert_records_sql")?,
        &insert,
    )?;
    execute_sql((execute, connection), insert_sql)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_source_freshness_read_latest_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_source_freshness_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(read_latest_source_freshness, module)?)?;
    module.add_function(wrap_pyfunction!(write_source_freshness_records, module)?)?;
    Ok(())
}
