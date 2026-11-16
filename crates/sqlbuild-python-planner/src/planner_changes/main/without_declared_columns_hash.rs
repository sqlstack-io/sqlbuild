//! Fingerprint metadata JSON without the declared-columns hash key, if present.

use pyo3::pyfunction;
use sqlbuild_analysis::planner_changes::main::without_declared_columns_hash::without_declared_columns_hash as without_hash;

/// Fingerprint metadata JSON without the declared-columns hash key, if present.
#[pyfunction]
pub fn without_declared_columns_hash(metadata_json: &str) -> String {
    without_hash(metadata_json)
}
