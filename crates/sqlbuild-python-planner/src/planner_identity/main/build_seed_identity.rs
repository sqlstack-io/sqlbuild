//! `(identity_hash, metadata_json)` for one compiled seed.

use crate::planner_identity::_helpers::metadata::seed_identity;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// `(identity_hash, metadata_json)` for one compiled seed.
#[pyfunction]
pub fn build_seed_identity(seed: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    seed_identity(seed)
}
