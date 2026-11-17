//! `value` as text, or `None` when it is `None`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `value` as text, or `None` when it is `None`.
pub fn nullable_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}
