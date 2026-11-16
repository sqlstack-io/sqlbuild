//! Validate the target warehouses and target connections a warehouse connection depends on.

use crate::compile_driver::_helpers::config_validation::reject_invalid_target_connections;
use crate::compile_driver::_helpers::config_validation::reject_unsupported_target_warehouses;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::PyResult;

/// Validate the target warehouses and target connections a warehouse connection depends on.
pub fn reject_invalid_connection_configuration(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    reject_unsupported_target_warehouses(config)?;
    reject_invalid_target_connections(config)
}
