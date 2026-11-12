//! DDL and pruning of the display-only observations table through the adapter.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::OBSERVATIONS_TABLE_NAME;
use sqlbuild_state::observations::constants::{
    NULLABLE_OBSERVATION_COLUMNS, OBSERVATION_COLUMN_TYPES,
};
use sqlbuild_state::observations::main::prune_observations_sql::prune_observations_sql;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;
use uuid::Uuid;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::consumption_state::{
    Location, ROWS_PER_STATEMENT, execute, literal, state_table_name,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::state_rows::state_tables::state_table_create_sql;

/// Create-if-missing DDL for the observations table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
fn build_observations_create_table_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py: Python<'_> = database.py();
    let Some(table) = state_table_name(
        render_qualified_name,
        (database, schema),
        OBSERVATIONS_TABLE_NAME,
    )?
    else {
        return Ok(String::new());
    };
    let columns: Bound<'_, PyAny> = PyTuple::new(
        py,
        OBSERVATION_COLUMN_TYPES.iter().map(|(column, _)| *column),
    )?
    .into_any();
    let types: Bound<'_, PyDict> = PyDict::new(py);
    for (column, declared) in OBSERVATION_COLUMN_TYPES {
        types.set_item(column, declared)?;
    }
    let required: Vec<&str> = OBSERVATION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .filter(|column| !NULLABLE_OBSERVATION_COLUMNS.contains(column))
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

/// SQL deleting observations beyond the newest `retain_versions` per kind and node.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
fn build_observations_prune_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    retain_versions: i64,
) -> PyResult<String> {
    Ok(state_table_name(
        render_qualified_name,
        (database, schema),
        OBSERVATIONS_TABLE_NAME,
    )?
    .map(|table| prune_observations_sql(&table, retain_versions))
    .unwrap_or_default())
}

/// One observation to append; the warehouse clock fills `observed_at`.
pub(crate) struct ObservationAppend {
    pub(crate) kind: &'static str,
    pub(crate) node: String,
    pub(crate) cursor_type: Option<String>,
    pub(crate) value: String,
    pub(crate) run_id: String,
}

/// Create the schema and observations table in `location` if missing, then append `rows`.
pub(crate) fn append_observations(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    rows: &[ObservationAppend],
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    let keywords = location.keywords(py);
    let Some(table) = state_table_name(
        &native_field(adapter, "render_qualified_name")?,
        (&keywords[0].1, &keywords[1].1),
        OBSERVATIONS_TABLE_NAME,
    )?
    else {
        return Ok(());
    };
    for statement in
        call_with(&native_field(adapter, "render_create_schema")?, &keywords)?.try_iter()?
    {
        execute(adapter, connection, &statement?.extract::<String>()?)?;
    }
    let create: String = call_with(
        &native_field(adapter, "render_create_observations_table_sql")?,
        &keywords,
    )?
    .extract()?;
    if !create.is_empty() {
        execute(adapter, connection, &create)?;
    }
    let observed_at: String =
        call_with(&native_field(adapter, "render_current_utc_timestamp")?, &[])?.extract()?;
    let columns: Vec<&str> = OBSERVATION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    for chunk in rows.chunks(ROWS_PER_STATEMENT) {
        let cells: Vec<Vec<String>> = chunk
            .iter()
            .map(|row| {
                Ok(vec![
                    literal(py, Some(&Uuid::now_v7().to_string()))?,
                    literal(py, Some(row.kind))?,
                    literal(py, Some(&row.node))?,
                    literal(py, row.cursor_type.as_deref())?,
                    literal(py, Some(&row.value))?,
                    literal(py, Some(&row.run_id))?,
                    observed_at.clone(),
                ])
            })
            .collect::<PyResult<_>>()?;
        execute(
            adapter,
            connection,
            &values_insert_sql(&table, &columns, &cells),
        )?;
    }
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_observations_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_observations_prune_sql, module)?)
}
