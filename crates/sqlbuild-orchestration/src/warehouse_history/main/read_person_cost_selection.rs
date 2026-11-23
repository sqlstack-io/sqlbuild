//! A selection's estimated query cost over a period, job by node, with daily series by job and top-level folder.

use chrono::Utc;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::cost_selection::{CostPeriod, selection_costs};
use crate::warehouse_history::_helpers::cost_summary::summarise;
use crate::warehouse_history::models::CostSelectionRecord;

/// The cost of the nodes `select` and `exclude` pick over the last `days` whole UTC days and the days before.
pub async fn read_person_cost_selection(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    ((select, exclude), days, default_role): ((&str, &str), u32, DefaultRole),
) -> Result<CostSelectionRecord, ControlError> {
    let period = CostPeriod::ending(Utc::now(), days);
    let costs = selection_costs(
        store,
        (planner, credentials),
        (location_id, target),
        ((select, exclude), &period, default_role),
    )
    .await?;
    Ok(summarise(&period, costs))
}
