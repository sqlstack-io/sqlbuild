//! Read a `dict[str, object]` in insertion order, leaving out keys no parameter can name.

use crate::attachments::_helpers::argument_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use sqlbuild_attachments::audits::models::ArgumentValue;

/// Read a `dict[str, object]` in insertion order, leaving out keys no parameter can name.
#[inline]
pub fn argument_pairs(arguments: &Bound<'_, PyDict>) -> PyResult<Vec<(String, ArgumentValue)>> {
    argument_values::argument_pairs(arguments)
}
