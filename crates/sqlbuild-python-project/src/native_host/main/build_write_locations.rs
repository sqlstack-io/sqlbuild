//! The schemas a build writes, which each record its run.

use crate::native_host::_helpers::run_locations::{run_locations, written_schema};
use crate::native_host::constants::SKIP_ACTION;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::Location;
use std::collections::BTreeSet;

/// Where a build records: its models' and seeds' schemas that run, its functions', its sources'.
pub fn build_write_locations(
    plan: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<Location>> {
    let mut schemas: BTreeSet<Location> = BTreeSet::new();
    for entries in ["model_entries", "seed_entries"] {
        for entry in native_field(plan, entries)?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            if !native_field(&entry, "action")?.eq(SKIP_ACTION)? {
                schemas.extend(written_schema(&native_field(&entry, "destination")?)?);
            }
        }
    }
    for entry in native_field(plan, "function_entries")?.try_iter()? {
        schemas.extend(written_schema(&native_field(&entry?, "destination")?)?);
    }
    let sources: Bound<'_, PyAny> = native_field(plan, "source_map")?;
    for entry in native_field(plan, "source_load_entries")?.try_iter()? {
        if let Some(source) = sources
            .call_method1("get", (native_field(&entry?, "name")?,))?
            .extract::<Option<Bound<'_, PyAny>>>()?
        {
            schemas.extend(written_schema(&source)?);
        }
    }
    run_locations(schemas, (plan, project))
}
