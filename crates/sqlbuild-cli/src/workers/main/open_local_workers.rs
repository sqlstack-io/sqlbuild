//! Open the installation's local process owner before accepting scheduler work.

use sqlbuild_orchestration::store::errors::ControlError;

use crate::workers::_helpers::configuration::read_configuration;
use crate::workers::models::LocalWorkers;

pub(crate) fn open_local_workers(
    configuration_path: Option<&str>,
    control_variable: String,
    scheduler_enabled: bool,
) -> Result<LocalWorkers, ControlError> {
    Ok(LocalWorkers {
        configuration: read_configuration(configuration_path, scheduler_enabled)?,
        active: Vec::new(),
        control_variable,
    })
}
