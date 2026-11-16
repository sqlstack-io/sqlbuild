//! Every event of `layout`'s table this version reads from one schema, from `stored` columns when known.

use crate::state_rows::_helpers::migrations::migration_io::execute;
use crate::state_rows::_helpers::migrations::migration_io::unreadable;
use crate::state_rows::_helpers::migrations::migration_rows::{decoded_events, read_statement};
use crate::state_rows::models::{EventTableLayout, StateAccess};
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;

/// Every event of `layout`'s table this version reads from one schema, from `stored` columns when known.
pub fn read_events<'py>(
    access: &StateAccess<'py>,
    layout: &EventTableLayout,
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    stored: Option<&[String]>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = schema.py();
    let (sql, columns): (String, Vec<String>) = read_statement(
        layout,
        &access.render_qualified_name,
        (database, schema.clone()),
        stored,
    )?;
    let rows: Bound<'py, PyAny> =
        match execute(access, &sql).and_then(|cursor| cursor.call_method0("fetchall")) {
            Ok(rows) => rows,
            Err(error) if error.is_instance_of::<PyException>(py) => {
                return Err(unreadable(layout, &schema, error));
            }
            Err(error) => return Err(error),
        };
    decoded_events(layout.table, &rows, &columns)
}
