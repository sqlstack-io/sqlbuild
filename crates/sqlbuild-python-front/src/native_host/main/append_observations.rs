//! Create the schema and observations table in `location` if missing, then append `rows`.

use crate::native_host::constants::ROWS_PER_STATEMENT;
use crate::native_host::main::execute::execute;
use crate::native_host::main::literal::literal;
use crate::native_host::main::state_table_name::state_table_name;
use crate::native_host::models::Location;
use crate::native_host::models::ObservationAppend;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_core::constants::OBSERVATIONS_TABLE_NAME;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::observations::constants::OBSERVATION_COLUMN_TYPES;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;
use uuid::Uuid;

/// Create the schema and observations table in `location` if missing, then append `rows`.
pub fn append_observations(
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
