//! `profile.function_return_types` items in order.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;

/// `profile.function_return_types` items in order.
#[inline]
pub fn return_types(profile: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    profile_inputs::return_types(profile)
}
