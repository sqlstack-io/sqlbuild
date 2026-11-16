//! Every name whose rename identity can change the migration fingerprint.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::planner_migrations::main::migration_ref_names;

/// Every name whose rename identity can change the migration fingerprint.
pub fn migration_fingerprint_ref_names<'py>(
    py: Python<'py>,
    query_sql: &str,
    metadata_json: &str,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    PyFrozenSet::new(
        py,
        migration_ref_names::migration_ref_names(query_sql, metadata_json)
            .map_err(PyValueError::new_err)?,
    )
}
