//! A target's estimated query cost over recent days, by job, warehouse size and node.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::costs::{
    Accumulator, COMPLETE_STATUS, TOTAL_RESOURCE_TYPE, latest_costs_in, ranked,
};
use crate::warehouse_history::models::{CostGroup, CostSummaryRecord};

/// Cost of the runs that started in the last `days` days; empty groups before any cost synced.
pub async fn read_person_cost_summary(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    (days, default_role): (u32, DefaultRole),
) -> Result<CostSummaryRecord, ControlError> {
    let since = Utc::now() - Duration::days(i64::from(days));
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, costs) =
        latest_costs_in(transaction, (location_id, target), since, None).await?;
    commit(transaction).await?;
    let mut total = Accumulator::default();
    let mut incomplete_runs: i64 = 0;
    let mut by_job: BTreeMap<Option<String>, Accumulator> = BTreeMap::new();
    let mut by_size: BTreeMap<Option<String>, Accumulator> = BTreeMap::new();
    let mut by_node: BTreeMap<Option<String>, Accumulator> = BTreeMap::new();
    for cost in &costs {
        let record = &cost.record;
        if record.resource_type == TOTAL_RESOURCE_TYPE {
            total.add(cost);
            incomplete_runs += i64::from(record.status != COMPLETE_STATUS);
            by_job.entry(cost.job_key.clone()).or_default().add(cost);
            continue;
        }
        by_size
            .entry(Some(record.warehouse_size.clone()).filter(|size| !size.is_empty()))
            .or_default()
            .add(cost);
        by_node
            .entry(Some(format!(
                "{}:{}",
                record.resource_type, record.resource_name
            )))
            .or_default()
            .add(cost);
    }
    let total: CostGroup = total.group(None);
    Ok(CostSummaryRecord {
        since,
        credits: total.credits,
        usd: total.usd,
        runs: total.runs,
        incomplete_runs,
        by_job: ranked(by_job),
        by_warehouse_size: ranked(by_size),
        by_node: ranked(by_node),
    })
}
