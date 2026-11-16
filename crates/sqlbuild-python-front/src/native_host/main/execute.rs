//! Execute.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Execute.
#[inline]
pub fn execute<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    sql: &str,
) -> PyResult<Bound<'py, PyAny>> {
    consumption_state::execute(adapter, connection, sql)
}
