//! Open the installation's process owner before accepting scheduler work.

use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::workers::_helpers::configuration::{resolve_locations, validate_configuration};
use crate::workers::models::{Launchers, Workers};

/// Validate the configured launchers, then resolve each location name in the opened store.
pub(crate) async fn open_workers(
    store: &ControlStore,
    mut configuration: Launchers,
    (control_variable, scheduler_enabled): (String, bool),
) -> Result<Workers, ControlError> {
    validate_configuration(&configuration, scheduler_enabled)?;
    resolve_locations(store, &mut configuration).await?;
    Ok(Workers {
        configuration,
        active: Vec::new(),
        control_variable,
        recovered_epoch: None,
    })
}
