//! `(identity_hash, metadata_json)` for one compiled seed.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `(identity_hash, metadata_json)` for one compiled seed.
#[inline]
pub fn build_seed_identity(seed: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    metadata::build_seed_identity(seed)
}
