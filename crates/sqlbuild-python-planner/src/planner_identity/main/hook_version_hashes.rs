//! `{name: identity.version_hash}` of the discovered Python hooks.

use crate::planner_identity::_helpers::identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// `{name: identity.version_hash}` of the discovered Python hooks.
#[inline]
pub fn hook_version_hashes<'py>(
    hook_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    identities::hook_version_hashes(hook_functions)
}
