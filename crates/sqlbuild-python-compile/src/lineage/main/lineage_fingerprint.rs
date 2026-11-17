//! The cache key over authored inputs and `cli_vars`, or `None` when they cannot be hashed.

use crate::lineage::_helpers::lineage::lineage_cache;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The cache key over authored inputs and `cli_vars`, or `None` when they cannot be hashed.
#[inline]
pub fn lineage_fingerprint(
    project_dir: &Bound<'_, PyAny>,
    cli_vars: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    lineage_cache::lineage_fingerprint(project_dir, cli_vars)
}
