//! The schemas a run writes, where it records its rows in the runs table.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::Location;

/// The schema of a relation the run writes; None for a relation without one.
pub(crate) fn written_schema(destination: &Bound<'_, PyAny>) -> PyResult<Option<Location>> {
    let schema: Option<String> = native_field(destination, "schema")?.extract()?;
    schema
        .map(|schema| {
            Ok(Location {
                database: native_field(destination, "database")?.extract()?,
                schema,
            })
        })
        .transpose()
}
