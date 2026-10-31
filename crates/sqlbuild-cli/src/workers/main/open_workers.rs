//! Open the installation's process owner before accepting scheduler work.

use sqlbuild_orchestration::store::errors::ControlError;

use crate::workers::_helpers::configuration::read_configuration;
use crate::workers::models::Workers;

pub(crate) fn open_workers(
    configuration_path: Option<&str>,
    control_variable: String,
    scheduler_enabled: bool,
) -> Result<Workers, ControlError> {
    Ok(Workers {
        configuration: read_configuration(configuration_path, scheduler_enabled)?,
        active: Vec::new(),
        control_variable,
        recovered_epoch: None,
    })
}
