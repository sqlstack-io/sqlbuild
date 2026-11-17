//! The column names of one schema's old-name view state table.

use crate::state_rows::main::migrations::stored_old_name_columns::stored_old_name_columns;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

/// The column names of one schema's old-name view state table.
#[pyfunction(name = "stored_old_name_view_columns")]
#[pyo3(signature = (*, adapter, connection, database, schema))]
pub fn stored_columns_binding<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    database: Bound<'py, PyAny>,
    schema: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(
        adapter.py(),
        stored_old_name_columns(adapter, connection, (database, schema))?,
    )
}
