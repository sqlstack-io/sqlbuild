//! The environment variables of the class's required secrets the environment leaves unset; an invalid setting is an error.

use crate::orchestration_reads::_helpers::provider_settings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The environment variables of the class's required secrets the environment leaves unset; an invalid setting is an error.
#[inline]
pub fn missing_secrets(provider_class: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    provider_settings::missing_secrets(provider_class)
}
