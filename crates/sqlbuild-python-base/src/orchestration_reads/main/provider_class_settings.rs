//! The class's description and declared setting fields, read without any setting value.

use crate::orchestration_reads::_helpers::provider_settings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use serde_json::Value;

/// The class's description and declared setting fields, read without any setting value.
#[inline]
pub fn provider_class_settings(provider_class: &Bound<'_, PyAny>) -> PyResult<Value> {
    provider_settings::provider_class_settings(provider_class)
}
