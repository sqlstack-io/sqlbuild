//! Append-only state table DDL and the source freshness insert, rendered through adapter callbacks.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_state::freshness::constants::{
    SOURCE_FRESHNESS_COLUMN_TYPES, SOURCE_FRESHNESS_EVENT_ID, STRING_STATE_VALUE_TYPE,
    TEXT_TIMESTAMP_STATE_VALUE_TYPE,
};
use sqlbuild_state::state_tables::main::create_table_sql::create_table_sql;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;
use sqlbuild_state::state_tables::models::TableColumn;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::state_rows::freshness::record_event_id;

const MISSING_SCHEMA_MESSAGE: &str = "source freshness table requires a target schema";
/// Each state value type a state table may declare and the framework type that renders it.
const DECLARED_TYPES: [(&str, &str); 3] = [
    (STRING_STATE_VALUE_TYPE, "STRING"),
    ("integer", "INTEGER"),
    (TEXT_TIMESTAMP_STATE_VALUE_TYPE, "TIMESTAMP"),
];

/// Create-if-missing DDL whose column types the adapter renders.
pub(crate) fn state_table_create_sql<'py>(
    qualified_name: &str,
    (columns, column_types, required_columns): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    render_framework_type: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let py: Python<'py> = render_framework_type.py();
    let framework_types: Bound<'py, PyAny> = host::entry(py, AdapterHost::FrameworkType)?;
    let mut rendered: Vec<(&str, String)> = Vec::with_capacity(DECLARED_TYPES.len());
    for (declared, framework) in DECLARED_TYPES {
        let text: String = render_framework_type
            .call1((native_field(&framework_types, framework)?,))?
            .extract()?;
        rendered.push((declared, text));
    }
    let mut definitions: Vec<TableColumn> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        let declared: Bound<'py, PyAny> = column_types.get_item(&column)?;
        let declared_text: String = declared.str()?.to_str()?.to_owned();
        let Some((_, rendered_type)) = rendered.iter().find(|(name, _)| *name == declared_text)
        else {
            return Err(PyKeyError::new_err(declared.unbind()));
        };
        definitions.push(TableColumn {
            name: column.extract()?,
            rendered_type: rendered_type.clone(),
            required: required_columns.contains(&column)?,
        });
    }
    Ok(create_table_sql(qualified_name, &definitions))
}

/// Render create-if-missing DDL for string, integer, and text-timestamp state columns.
#[pyfunction(name = "render_state_table_create_sql")]
#[pyo3(signature = (**request))]
fn render_create_sql(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("render_state_table_create_sql", request)?;
    state_table_create_sql(
        &request.get("qualified_name")?.extract::<String>()?,
        (
            &request.get("columns")?,
            &request.get("column_types")?,
            &request.get("required_columns")?,
        ),
        &request.get("render_framework_type")?,
    )
}

fn adapter_user_error(py: Python<'_>, message: &str) -> PyErr {
    match host::entry(py, AdapterHost::AdapterUserError).and_then(|class| {
        call_with(
            &class,
            &[("message", PyString::new(py, message).into_any())],
        )
    }) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// Render DML that appends source freshness records.
#[pyfunction]
#[pyo3(signature = (*, database, schema, records, render_qualified_name))]
fn render_insert_source_freshness_records_sql<'py>(
    database: &Bound<'py, PyAny>,
    schema: &Bound<'py, PyAny>,
    records: &Bound<'py, PyAny>,
    render_qualified_name: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let py: Python<'py> = database.py();
    let table: Bound<'py, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            (
                "name",
                PyString::new(py, SOURCE_FRESHNESS_TABLE_NAME).into_any(),
            ),
        ],
    )?;
    if table.is_none() {
        return Err(adapter_user_error(py, MISSING_SCHEMA_MESSAGE));
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    for record in records.try_iter()? {
        let record: Bound<'py, PyAny> = record?;
        let mut row: Vec<String> = SOURCE_FRESHNESS_COLUMN_TYPES
            .iter()
            .map(|(column, declared)| {
                render_state_sql_literal(py, &native_field(&record, column)?, declared, None)
            })
            .collect::<PyResult<_>>()?;
        row.push(format!("'{}'", record_event_id(&record)?));
        rows.push(row);
    }
    let mut names: Vec<&str> = SOURCE_FRESHNESS_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    names.push(SOURCE_FRESHNESS_EVENT_ID);
    Ok(values_insert_sql(
        &table.extract::<String>()?,
        &names,
        &rows,
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_create_sql, module)?)?;
    module.add_function(wrap_pyfunction!(
        render_insert_source_freshness_records_sql,
        module
    )?)
}
