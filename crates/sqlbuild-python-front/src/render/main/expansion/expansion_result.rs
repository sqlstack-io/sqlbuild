//! Expansion result.

use crate::render::_helpers::expansion::macro_expansion_result;
use crate::render::models::ExpansionRequest;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Expansion result.
#[inline]
pub fn expansion_result<'py>(request: ExpansionRequest<'py>) -> PyResult<Bound<'py, PyAny>> {
    macro_expansion_result::expansion_result(request)
}
