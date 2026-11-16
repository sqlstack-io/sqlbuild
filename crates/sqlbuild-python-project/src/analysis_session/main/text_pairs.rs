//! `{column: value}` as ordered pairs, each value as `str(value)`.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;

/// `{column: value}` as ordered pairs, each value as `str(value)`.
#[inline]
pub fn text_pairs(shape: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    profile_inputs::text_pairs(shape)
}
