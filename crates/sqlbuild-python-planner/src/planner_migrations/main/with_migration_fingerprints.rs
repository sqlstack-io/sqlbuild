//! Attach persisted migration identities through the existing cache boundary.

use crate::planner_migrations::_helpers::projections::fingerprint;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// Attach persisted migration identities through the existing cache boundary.
#[inline]
pub fn with_migration_fingerprints<'py>(
    entries: &Bound<'py, PyAny>,
    models: &ModelLookup,
    cache: &Bound<'py, PyAny>,
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    fingerprint::with_migration_fingerprints(entries, models, cache, dialect)
}
