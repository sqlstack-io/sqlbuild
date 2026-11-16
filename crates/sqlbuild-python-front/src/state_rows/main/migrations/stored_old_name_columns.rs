//! The column names of one schema's old-name view state table, as the adapter lists them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The column names of one schema's old-name view state table, as the adapter lists them.
pub fn stored_old_name_columns<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Vec<String>> {
    let py: Python<'py> = adapter.py();
    let columns: Bound<'py, PyAny> = call_with(
        &native_field(adapter, "get_columns")?,
        &[
            ("connection", connection.clone()),
            ("database", database),
            ("schema", schema),
            (
                "name",
                PyString::new(py, OLD_NAME_VIEW_TABLE_NAME).into_any(),
            ),
        ],
    )?;
    columns
        .try_iter()?
        .map(|column| native_field(&column?, "name")?.extract())
        .collect()
}
