//! The adapter's qualified name of a state table in `(database, schema)`, if it has one.

use crate::native_host::_helpers::consumption_state::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::call_with::call_with;

/// The adapter's qualified name of a state table in `(database, schema)`, if it has one.
pub fn state_table_name(
    render_qualified_name: &Bound<'_, PyAny>,
    (database, schema): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    table: &str,
) -> PyResult<Option<String>> {
    let py: Python<'_> = render_qualified_name.py();
    let name: Bound<'_, PyAny> = call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", text(py, table)),
        ],
    )?;
    if name.is_none() {
        return Ok(None);
    }
    Ok(Some(name.extract()?))
}
