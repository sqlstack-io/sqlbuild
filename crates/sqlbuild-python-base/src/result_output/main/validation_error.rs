//! The `ObservabilityValidationError` Python raised for the same contract failure.

use crate::result_output::_helpers::integration_values;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;
use sqlbuild_core::integration_results::errors::IntegrationRecordError;

/// The `ObservabilityValidationError` Python raised for the same contract failure.
#[inline]
pub fn validation_error(py: Python<'_>, error: &IntegrationRecordError) -> PyErr {
    integration_values::validation_error(py, error)
}
