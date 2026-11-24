//! Daily cost series of a selection, for a batch of groups by job, folder and node.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::cost_selection::selection_costs_in_days;
use crate::warehouse_history::_helpers::cost_summary::group_series;
use crate::warehouse_history::constants::COST_SERIES_GROUPS;
use crate::warehouse_history::models::{CostSeriesBatch, CostSeriesGroup};

/// Each group's credits per day of the last `days` whole UTC days, for the nodes `select` and `exclude` pick.
pub async fn read_person_cost_series(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    ((select, exclude), (days, groups), default_role): (
        (&str, &str),
        (u32, &[CostSeriesGroup]),
        DefaultRole,
    ),
) -> Result<CostSeriesBatch, ControlError> {
    if groups.len() > COST_SERIES_GROUPS {
        return Err(ControlError::Invalid(format!(
            "at most {COST_SERIES_GROUPS} groups per request"
        )));
    }
    let (period, costs) = selection_costs_in_days(
        store,
        (planner, credentials),
        (location_id, target),
        ((select, exclude), days, default_role),
    )
    .await?;
    Ok(CostSeriesBatch {
        dates: period.dates(),
        series: group_series(&period, &costs.rows, groups),
    })
}
