//! The migration fingerprint of SQL and metadata through rename identities, `generic` by default.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::planner_migrations::main::migration_fingerprint;
use std::collections::BTreeMap;

/// The migration fingerprint of SQL and metadata through rename identities, `generic` by default.
pub fn fingerprint(
    sql: &str,
    metadata: &str,
    identities: &BTreeMap<String, String>,
    dialect: Option<&str>,
) -> PyResult<Option<String>> {
    migration_fingerprint::migration_fingerprint(
        sql,
        metadata,
        identities,
        dialect
            .filter(|value| !value.is_empty())
            .unwrap_or("generic"),
    )
    .map_err(PyValueError::new_err)
}
