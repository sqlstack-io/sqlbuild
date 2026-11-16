//! A config value when it is text.

use crate::compiled_project::_helpers::project;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// A config value when it is text.
#[inline]
pub fn text_value(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    project::text_value(values, key)
}
