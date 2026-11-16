//! Instantiate the class, telling unset required secrets apart from other invalid settings.

use crate::orchestration_reads::_helpers::provider_settings;
use crate::orchestration_reads::models::ProviderState;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Instantiate the class, telling unset required secrets apart from other invalid settings.
#[inline]
pub fn provider_state<'py>(provider_class: &Bound<'py, PyAny>) -> PyResult<ProviderState<'py>> {
    provider_settings::provider_state(provider_class)
}
