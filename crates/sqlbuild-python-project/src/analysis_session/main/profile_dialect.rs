//! `profile.sql_analysis_dialect or "generic"`.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `profile.sql_analysis_dialect or "generic"`.
#[inline]
pub fn profile_dialect(profile: &Bound<'_, PyAny>) -> PyResult<String> {
    profile_inputs::profile_dialect(profile)
}
