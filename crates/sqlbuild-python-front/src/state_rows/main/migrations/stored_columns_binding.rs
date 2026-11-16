//! The column names of one schema's old-name view state table.

use crate::state_rows::_helpers::migrations::migration_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// The column names of one schema's old-name view state table.
#[inline]
pub fn stored_columns_binding<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    database: Bound<'py, PyAny>,
    schema: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    migration_io::stored_columns_binding(adapter, connection, database, schema)
}
