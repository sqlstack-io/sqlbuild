//! A model's config values as typed nodes, read key by key.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A model's config values as typed nodes, read key by key.
#[inline]
pub fn model_values<'py>(model: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    typed_config::model_values(model)
}
