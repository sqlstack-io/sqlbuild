//! `values[key]` as text when it is present and a `str`, else `None`.

use crate::boundary::main::optional_text::optional_text;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

/// `values[key]` as text when it is present and a `str`, else `None`.
pub fn text_item(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) => optional_text(&value),
        None => Ok(None),
    }
}
