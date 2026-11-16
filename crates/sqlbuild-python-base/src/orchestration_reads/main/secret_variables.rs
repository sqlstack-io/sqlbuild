//! The environment variables of the class's secret settings.

use crate::orchestration_reads::_helpers::provider_settings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The environment variables of the class's secret settings.
#[inline]
pub fn secret_variables(provider_class: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    provider_settings::secret_variables(provider_class)
}
