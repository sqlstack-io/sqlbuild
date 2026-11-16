//! Every name whose rename identity can change the migration fingerprint.

use crate::planner_migrations::_helpers::projections::fingerprint;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyFrozenSet;

/// Every name whose rename identity can change the migration fingerprint.
#[inline]
pub fn migration_fingerprint_ref_names<'py>(
    py: Python<'py>,
    query_sql: &str,
    metadata_json: &str,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    fingerprint::migration_fingerprint_ref_names(py, query_sql, metadata_json)
}
