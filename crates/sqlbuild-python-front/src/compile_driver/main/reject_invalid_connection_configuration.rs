//! Validate the target warehouses, target connections and source connections of a project.

use crate::compile_driver::_helpers::config_validation::reject_invalid_target_connections;
use crate::compile_driver::_helpers::config_validation::reject_unsupported_target_warehouses;
use crate::compile_driver::_helpers::source_connections::reject_invalid_source_connections;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::PyResult;

/// Validate the target warehouses, target connections and source connections of a project.
pub fn reject_invalid_connection_configuration(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    reject_unsupported_target_warehouses(config)?;
    reject_invalid_target_connections(config)?;
    reject_invalid_source_connections(config)
}
