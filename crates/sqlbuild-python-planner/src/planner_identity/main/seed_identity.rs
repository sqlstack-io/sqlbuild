//! `(identity_hash, metadata_json)` of one compiled seed from its content and load-affecting config.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `(identity_hash, metadata_json)` of one compiled seed from its content and load-affecting config.
#[inline]
pub fn seed_identity(seed: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    metadata::seed_identity(seed)
}
