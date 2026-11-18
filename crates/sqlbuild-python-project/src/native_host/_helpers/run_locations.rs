//! The schemas a run writes, where it records its rows in the runs table, each physical schema once.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::analysis_route::main::same_unquoted_name::same_unquoted_name;
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_state::runs::main::distinct_run_schemas::distinct_run_schemas;
use sqlbuild_state::runs::models::RunSchema;
use std::collections::BTreeSet;

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

/// A target namespace value, None when the target keeps each resource's own (`preserve`).
fn target_value(project: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<String>> {
    Ok(native_field(project, field)?
        .extract::<Option<String>>()?
        .filter(|value| value != PRESERVE_TARGET_VALUE))
}

/// Where a run writing no relation records, as audits do: the target's schema, else the first node's.
fn default_location(
    plan: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Option<Location>> {
    let database: Option<String> = target_value(project, "effective_target_database")?;
    if let Some(schema) = target_value(project, "effective_target_schema")? {
        return Ok(Some(Location { database, schema }));
    }
    for locations in ["model_locations", "seed_locations"] {
        for location in native_field(plan, locations)?
            .call_method0("values")?
            .try_iter()?
        {
            if let Some(found) = written_schema(&location?)? {
                return Ok(Some(Location {
                    database: database.or(found.database),
                    schema: found.schema,
                }));
            }
        }
    }
    Ok(None)
}

/// Each physical schema of `schemas` once, as the dialect resolves names; else the default location.
pub(crate) fn run_locations(
    schemas: BTreeSet<Location>,
    (plan, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Vec<Location>> {
    if schemas.is_empty() {
        return Ok(default_location(plan, project)?.into_iter().collect());
    }
    let dialect: Option<String> = native_field(project, "sql_analysis_dialect")?.extract()?;
    let target_database: Option<String> = target_value(project, "effective_target_database")?;
    let same_name = |left: &str, right: &str| same_unquoted_name(dialect.as_deref(), left, right);
    Ok(distinct_run_schemas(
        schemas
            .into_iter()
            .map(|location| RunSchema {
                database: location.database,
                schema: location.schema,
            })
            .collect(),
        target_database.as_deref(),
        &same_name,
    )
    .into_iter()
    .map(|schema| Location {
        database: schema.database,
        schema: schema.schema,
    })
    .collect())
}
