//! Create-if-missing DDL for a state table with fixed columns, typed by the adapter.

use crate::native_host::main::state_table_name::state_table_name;
use crate::state_rows::main::tables::state_table_create_sql::state_table_create_sql;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyTuple};

/// Create-if-missing DDL for a state table with fixed columns, typed by the adapter.
pub fn fixed_state_table_sql(
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
