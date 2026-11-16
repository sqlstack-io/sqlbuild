//! Execute.

use crate::native_host::_helpers::consumption_state::text;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Execute.
pub fn execute<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    sql: &str,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &native_field(adapter, "execute")?,
        &[
            ("connection", connection.clone()),
            ("sql", text(adapter.py(), sql)),
        ],
    )
}
