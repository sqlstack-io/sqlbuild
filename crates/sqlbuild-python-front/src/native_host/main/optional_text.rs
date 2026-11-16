//! Optional text.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// Optional text.
pub fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}
