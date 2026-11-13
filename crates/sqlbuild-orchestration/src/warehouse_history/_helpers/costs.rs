//! The latest estimated query cost observations of a target, each with the job of its server run.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::models::{CostGroup, NodeCostRecord};
use crate::warehouse_metadata::models::WarehouseResourceCostRecord;

/// Total rows carry a build's totals; every other row is one resource on one warehouse.
pub(crate) const TOTAL_RESOURCE_TYPE: &str = "_total";
/// Statuses of a build whose cost may still change: a later build re-reads it.
pub(crate) const PENDING_STATUSES: [&str; 2] = ["pending", "partial"];

/// Every row of the run's newest attempt; a refresh appends a whole attempt, so older ones stay.
pub(crate) const RUN_COST_SQL: &str = "SELECT record_json FROM (SELECT record_json, attempt, \
    MAX(attempt) OVER (PARTITION BY run_id) AS newest FROM sqb_warehouse_resource_costs \
    WHERE location_id=$1 AND target=$2 AND orchestration_run_id=$3) costs WHERE attempt=newest \
    ORDER BY record_json ->> 'resource_type', record_json ->> 'resource_name'";

/// Rows of each run's newest attempt since `$3`, optionally only a node's (`$4`, `$5`) and totals.
const LATEST_COSTS_SQL: &str = "SELECT c.record_json, r.job_key FROM (SELECT record_json, \
    orchestration_run_id, attempt, MAX(attempt) OVER (PARTITION BY run_id) AS newest \
    FROM sqb_warehouse_resource_costs WHERE location_id=$1 AND target=$2 AND run_started_at >= $3 \
    AND (CAST($4 AS TEXT) IS NULL OR resource_type IN ('_total', $4)) \
    AND (CAST($5 AS TEXT) IS NULL OR resource_type='_total' OR resource_name=$5)) c \
    LEFT JOIN sqb_runs r ON CAST(r.run_id AS TEXT)=c.orchestration_run_id WHERE c.attempt=c.newest";

/// One latest observation and the job of the server run that built it, if any.
pub(crate) struct LatestCost {
    pub(crate) record: WarehouseResourceCostRecord,
    pub(crate) job_key: Option<String>,
}

/// Latest observations of runs started since `since`; with `node`, only its rows and total rows.
pub(crate) async fn latest_costs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    since: DateTime<Utc>,
    node: Option<(&str, &str)>,
) -> Result<(ControlTransaction, Vec<LatestCost>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        LATEST_COSTS_SQL,
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Time(since),
            Parameter::OptionalText(node.map(|(kind, _)| kind.to_owned())),
            Parameter::OptionalText(node.map(|(_, name)| name.to_owned())),
        ],
    )
    .await?;
    let costs = rows
        .into_iter()
        .map(|row| {
            Ok(LatestCost {
                record: serde_json::from_value(row["record_json"].clone())
                    .map_err(|error| ControlError::Invalid(error.to_string()))?,
                job_key: row["job_key"].as_str().map(str::to_owned),
            })
        })
        .collect::<Result<_, ControlError>>()?;
    Ok((transaction, costs))
}

/// A decimal text as a number; an unreadable value counts as zero.
pub(crate) fn amount(text: &str) -> f64 {
    text.trim().parse().unwrap_or(0.0)
}

/// Running totals of one cost group.
#[derive(Default)]
pub(crate) struct Accumulator {
    credits: f64,
    usd: f64,
    /// An observation had no configured credit price, so the dollar total would understate.
    unpriced: bool,
    seconds: f64,
    runs: BTreeSet<String>,
}

impl Accumulator {
    pub(crate) fn add(&mut self, cost: &LatestCost) {
        let record = &cost.record;
        self.credits += amount(&record.estimated_credits);
        match record.estimated_usd.as_deref() {
            Some(usd) => self.usd += amount(usd),
            None => self.unpriced = true,
        }
        self.seconds += amount(&record.attributed_seconds);
        self.runs.insert(record.run_id.clone());
    }

    pub(crate) fn group(self, key: Option<String>) -> CostGroup {
        CostGroup {
            key,
            credits: self.credits,
            usd: (!self.unpriced && !self.runs.is_empty()).then_some(self.usd),
            seconds: self.seconds,
            runs: i64::try_from(self.runs.len()).unwrap_or(i64::MAX),
        }
    }
}

/// Groups as records, most expensive first.
pub(crate) fn ranked(groups: BTreeMap<Option<String>, Accumulator>) -> Vec<CostGroup> {
    let mut ranked: Vec<CostGroup> = groups
        .into_iter()
        .map(|(key, accumulator)| accumulator.group(key))
        .collect();
    ranked.sort_by(|left, right| right.credits.total_cmp(&left.credits));
    ranked
}

/// The node's rows folded into one record per build with its status, newest first, at most `limit`.
pub(crate) fn node_builds(costs: &[LatestCost], limit: u32) -> Vec<NodeCostRecord> {
    let mut builds: BTreeMap<String, NodeCostRecord> = BTreeMap::new();
    let mut statuses: BTreeMap<String, String> = BTreeMap::new();
    for cost in costs {
        let record = &cost.record;
        if record.resource_type == TOTAL_RESOURCE_TYPE {
            statuses.insert(record.run_id.clone(), record.status.clone());
            continue;
        }
        let build = builds
            .entry(record.run_id.clone())
            .or_insert_with(|| NodeCostRecord {
                run_id: record.run_id.clone(),
                orchestration_run_id: record.orchestration_run_id.clone(),
                run_started_at: record.run_started_at,
                status: String::new(),
                credits: 0.0,
                usd: Some(0.0),
                seconds: 0.0,
                query_count: 0,
            });
        build.credits += amount(&record.estimated_credits);
        build.usd = build
            .usd
            .zip(record.estimated_usd.as_deref())
            .map(|(total, usd)| total + amount(usd));
        build.seconds += amount(&record.attributed_seconds);
        build.query_count += record.query_count;
    }
    let mut records: Vec<NodeCostRecord> = builds
        .into_values()
        .map(|mut build| {
            build.status = statuses.remove(&build.run_id).unwrap_or_default();
            build
        })
        .collect();
    records.sort_by_key(|record| std::cmp::Reverse(record.run_started_at));
    records.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    records
}
