//! The generation a location's health report names, after its target and kind are checked.

use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;

/// `None` for a heartbeat; an error for a blank target, a generation below 1, or a kind only the server derives.
pub(crate) fn reported_generation(
    report: &LocationHealthReport,
) -> Result<Option<i64>, ControlError> {
    if report.target.trim().is_empty() {
        return Err(ControlError::Invalid(
            "location health target is invalid".into(),
        ));
    }
    let generation = match report.health {
        LocationHealth::Heartbeat {} => None,
        LocationHealth::CompileFailed { generation, .. }
        | LocationHealth::Connection { generation, .. }
        | LocationHealth::ProviderSecrets { generation, .. }
        | LocationHealth::WarehouseSync { generation, .. } => Some(generation),
        LocationHealth::PoolCapped { .. } => {
            return Err(ControlError::Invalid(
                "the server derives pool_capped health; a location cannot report it".into(),
            ));
        }
    };
    if generation.is_some_and(|generation| generation < 1) {
        return Err(ControlError::Invalid(
            "location health generation must be positive".into(),
        ));
    }
    Ok(generation)
}
