//! `dialect or "generic"`.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `dialect or "generic"`.
#[inline]
pub fn dialect_or_generic(dialect: &Bound<'_, PyAny>) -> PyResult<String> {
    profile_inputs::dialect_or_generic(dialect)
}
