//! Deterministic non-query model fingerprint metadata JSON.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Deterministic non-query model fingerprint metadata JSON.
#[inline]
pub fn model_version_identity_metadata_json<'py>(
    model: &Bound<'py, PyAny>,
    function_local_hashes: Option<&Bound<'py, PyAny>>,
    hook_version_hashes: Option<&Bound<'py, PyAny>>,
) -> PyResult<String> {
    metadata::model_version_identity_metadata_json(
        model,
        function_local_hashes,
        hook_version_hashes,
    )
}
