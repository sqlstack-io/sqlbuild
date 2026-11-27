//! Read a `dict[str, object]` in insertion order, leaving out keys no parameter can name.

use crate::attachments::_helpers::argument_values::argument_value;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use sqlbuild_attachments::audits::models::ArgumentValue;

/// Read a `dict[str, object]` in insertion order, leaving out keys no parameter can name.
pub fn argument_pairs(arguments: &Bound<'_, PyDict>) -> PyResult<Vec<(String, ArgumentValue)>> {
    let mut pairs: Vec<(String, ArgumentValue)> = Vec::with_capacity(arguments.len());
    for (key, value) in arguments.iter() {
        if let Ok(key) = key.cast::<PyString>() {
            pairs.push((key.to_string_lossy().into_owned(), argument_value(&value)?));
        }
    }
    Ok(pairs)
}
