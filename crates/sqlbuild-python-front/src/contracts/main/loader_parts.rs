//! Loader parts.

use crate::contracts::_helpers::spec_resolution::LOADER_QUALIFIED_TABLE_PARTS;
use crate::contracts::_helpers::spec_resolution::LOADER_SCHEMA_TABLE_PARTS;
use crate::contracts::_helpers::spec_resolution::blank;
use crate::contracts::_helpers::spec_resolution::spec_error;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// Loader parts.
pub fn loader_parts<'py>(
    py: Python<'py>,
    destination: &str,
    (default_database, default_schema): (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    let parts: Vec<&str> = destination.split('.').collect();
    if !(1..=LOADER_QUALIFIED_TABLE_PARTS).contains(&parts.len())
        || parts.iter().any(|part| blank(part))
    {
        return Err(spec_error(
            py,
            PyString::new(
                py,
                &format!(
                    "Invalid loader destination '{destination}': expected 1 to 3 non-empty \
                 dot-separated parts (table, schema.table, or database.schema.table)"
                ),
            ),
        )?);
    }
    let none = || py.None().into_bound(py);
    let text = |part: &str| PyString::new(py, part).into_any();
    let (database, schema, table): (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>) =
        match parts.as_slice() {
            [table] => (
                default_database.unwrap_or_else(none),
                default_schema.unwrap_or_else(none),
                text(table),
            ),
            [schema, table] if parts.len() == LOADER_SCHEMA_TABLE_PARTS => (
                default_database.unwrap_or_else(none),
                text(schema),
                text(table),
            ),
            _ => (text(parts[0]), text(parts[1]), text(parts[2])),
        };
    new_record(
        py,
        RecordId::LoaderDestinationParts,
        (database, schema, table),
        None,
    )
}
