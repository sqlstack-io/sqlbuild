//! Validate the SQL analysis policy, target warehouses and target connections, in that order.

use crate::compile_driver::_helpers::config_validation;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::PyResult;

/// Validate the SQL analysis policy, target warehouses and target connections, in that order.
#[inline]
pub fn reject_invalid_project_configuration(config: &ProjectConfiguration<'_, '_>) -> PyResult<()> {
    config_validation::reject_invalid_project_configuration(config)
}
