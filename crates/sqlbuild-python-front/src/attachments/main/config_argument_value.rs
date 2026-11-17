//! Native-built argument containers need no intermediate Python list or tuple.

use crate::attachments::_helpers::argument_values;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_attachments::audits::models::ArgumentValue;

/// Native-built argument containers need no intermediate Python list or tuple.
#[inline]
pub fn config_argument_value(py: Python<'_>, value: &PyConfigNode) -> PyResult<ArgumentValue> {
    argument_values::config_argument_value(py, value)
}
