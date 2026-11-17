//! `dialect or "generic"`.

use crate::analysis_session::_helpers::profile_inputs::GENERIC_DIALECT;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `dialect or "generic"`.
pub fn dialect_or_generic(dialect: &Bound<'_, PyAny>) -> PyResult<String> {
    if dialect.is_truthy()? {
        return dialect.extract();
    }
    Ok(GENERIC_DIALECT.to_owned())
}
