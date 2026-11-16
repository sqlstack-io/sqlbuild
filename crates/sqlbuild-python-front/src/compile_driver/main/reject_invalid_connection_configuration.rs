//! Validate the target warehouses and target connections a warehouse connection depends on.

use crate::compile_driver::_helpers::config_validation;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::PyResult;

/// Validate the target warehouses and target connections a warehouse connection depends on.
#[inline]
pub fn reject_invalid_connection_configuration(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    config_validation::reject_invalid_connection_configuration(config)
}
