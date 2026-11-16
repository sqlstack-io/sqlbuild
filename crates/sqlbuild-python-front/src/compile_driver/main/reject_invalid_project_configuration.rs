//! Validate the SQL analysis policy, target warehouses and target connections, in that order.

use crate::compile_driver::_helpers::config_validation::reject_local_sql_analysis_opt_out;
use crate::compile_driver::main::reject_invalid_connection_configuration::reject_invalid_connection_configuration;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::PyResult;

/// Validate the SQL analysis policy, target warehouses and target connections, in that order.
pub fn reject_invalid_project_configuration(config: &ProjectConfiguration<'_, '_>) -> PyResult<()> {
    reject_local_sql_analysis_opt_out(config)?;
    reject_invalid_connection_configuration(config)
}
