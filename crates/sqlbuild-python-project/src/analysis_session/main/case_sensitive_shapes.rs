//! Python's `inferred_binding_shape` test for keeping authored identifier quoting.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Python's `inferred_binding_shape` test for keeping authored identifier quoting.
#[inline]
pub fn case_sensitive_shapes(
    profile: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    profile_inputs::case_sensitive_shapes(profile, dialect)
}
