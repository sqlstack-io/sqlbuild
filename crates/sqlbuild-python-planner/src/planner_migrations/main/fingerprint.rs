//! The migration fingerprint of SQL and metadata through rename identities, `generic` by default.

use crate::planner_migrations::_helpers::projections::fingerprint;
use pyo3::prelude::PyResult;
use std::collections::BTreeMap;

/// The migration fingerprint of SQL and metadata through rename identities, `generic` by default.
#[inline]
pub fn fingerprint(
    sql: &str,
    metadata: &str,
    identities: &BTreeMap<String, String>,
    dialect: Option<&str>,
) -> PyResult<Option<String>> {
    fingerprint::fingerprint(sql, metadata, identities, dialect)
}
