//! A tags value as texts: a list or tuple of tags, each as text.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyList, PyTuple};

/// The texts of a list or tuple of tags; anything else has none.
pub fn tag_texts(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
        return Ok(Vec::new());
    }
    value
        .try_iter()?
        .map(|item| item?.str()?.extract())
        .collect()
}
