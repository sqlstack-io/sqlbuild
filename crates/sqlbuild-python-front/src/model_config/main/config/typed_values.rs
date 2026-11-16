//! A config record's values, their typed nodes kept on the record so each key converts once.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A config record's values, their typed nodes kept on the record so each key converts once.
#[inline]
pub fn typed_values<'py>(config: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    typed_config::typed_values(config)
}
