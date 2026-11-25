//! An object's attribute as text, when it has one that isn't `None`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `name` of `value` as text, or `None` when it is absent or `None`.
pub fn attribute_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    match value.getattr_opt(name)? {
        Some(item) if !item.is_none() => Ok(Some(item.str()?.extract()?)),
        Some(_) | None => Ok(None),
    }
}
