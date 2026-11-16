//! The column names of one schema's old-name view state table, as the adapter lists them.

use crate::state_rows::_helpers::migrations::migration_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The column names of one schema's old-name view state table, as the adapter lists them.
#[inline]
pub fn stored_old_name_columns<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Vec<String>> {
    migration_io::stored_old_name_columns(adapter, connection, (database, schema))
}
