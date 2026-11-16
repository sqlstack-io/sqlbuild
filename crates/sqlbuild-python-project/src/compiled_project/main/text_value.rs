//! A config value when it is text.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString};

/// A config value when it is text.
pub fn text_value(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) if value.is_instance_of::<PyString>() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}
