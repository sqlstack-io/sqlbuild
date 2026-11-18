//! The schemas `sqb seed` writes, which each record its run.

use crate::native_host::_helpers::run_locations::written_schema;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::Location;
use std::collections::BTreeSet;

/// The schemas `sqb seed` writes: those of every seed it loads.
pub fn seed_write_locations(plan: &Bound<'_, PyAny>) -> PyResult<Vec<Location>> {
    let mut schemas: BTreeSet<Location> = BTreeSet::new();
    for entry in native_field(plan, "seed_entries")?.try_iter()? {
        schemas.extend(written_schema(&native_field(&entry?, "destination")?)?);
    }
    Ok(schemas.into_iter().collect())
}
