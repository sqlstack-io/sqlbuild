//! Deterministic non-query model fingerprint metadata JSON.

use crate::planner_identity::main::model_version_identity_metadata_json::model_version_identity_metadata_json;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Deterministic non-query model fingerprint metadata JSON.
#[pyfunction]
#[pyo3(signature = (*, model, function_local_hashes = None, hook_version_hashes = None))]
pub fn build_model_version_identity_metadata_json<'py>(
    model: &Bound<'py, PyAny>,
    function_local_hashes: Option<Bound<'py, PyAny>>,
    hook_version_hashes: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    model_version_identity_metadata_json(
        model,
        function_local_hashes.as_ref(),
        hook_version_hashes.as_ref(),
    )
}
