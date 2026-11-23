//! A selection's cost rows over a period and the one before it: each with its job, node, folder and day.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::main::requests::select_viewed_nodes::select_viewed_nodes;
use crate::runs::models::{SelectionProblem, ViewedSelection};
use crate::runs::types::CommandPlanner;
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::costs::{
    PENDING_STATUSES, TOTAL_RESOURCE_TYPE, amount, latest_costs_in,
};

/// The folder of nodes whose path the publication doesn't give, such as nodes no longer deployed.
pub(crate) const OTHER_FOLDER: &str = "other";
/// The layout root of SQL models, which folders leave out.
const MODELS_ROOT: &str = "sql/models";

/// A period of whole UTC days ending today, and the one before it.
pub(crate) struct CostPeriod {
    pub(crate) days: u32,
    pub(crate) first_day: NaiveDate,
    pub(crate) since: DateTime<Utc>,
    pub(crate) previous_since: DateTime<Utc>,
}

impl CostPeriod {
    pub(crate) fn ending(now: DateTime<Utc>, days: u32) -> Self {
        let first_day: NaiveDate = now.date_naive() - Duration::days(i64::from(days) - 1);
        let since: DateTime<Utc> = first_day.and_time(chrono::NaiveTime::MIN).and_utc();
        Self {
            days,
            first_day,
            since,
            previous_since: since - Duration::days(i64::from(days)),
        }
    }

    /// The period's days as `YYYY-MM-DD`, oldest first.
    pub(crate) fn dates(&self) -> Vec<String> {
        (0..i64::from(self.days))
            .map(|day| {
                (self.first_day + Duration::days(day))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .collect()
    }

    /// The day of the period a build started on; None for the period before.
    fn day(&self, started: DateTime<Utc>) -> Option<usize> {
        if started < self.since {
            return None;
        }
        let offset: u64 = (started.date_naive() - self.first_day)
            .num_days()
            .unsigned_abs();
        let day: usize = usize::try_from(offset).unwrap_or(usize::MAX);
        (day < self.days as usize).then_some(day)
    }
}

/// One cost row of a selected node.
pub(crate) struct CostRow {
    pub(crate) run_id: String,
    pub(crate) job: Option<String>,
    pub(crate) node_key: String,
    pub(crate) folder: String,
    /// The day of the period; None for a row of the period before.
    pub(crate) day: Option<usize>,
    pub(crate) credits: f64,
    pub(crate) usd: Option<f64>,
    pub(crate) seconds: f64,
    pub(crate) queries: i64,
    pub(crate) bytes_scanned: i64,
}

/// The selection's rows, the builds whose cost is still pending, and the publication evaluated.
pub(crate) struct SelectionCosts {
    pub(crate) snapshot_id: Option<Uuid>,
    pub(crate) error: Option<SelectionProblem>,
    pub(crate) rows: Vec<CostRow>,
    pub(crate) pending_runs: BTreeSet<String>,
}

/// A node's folder: its directory, without `sql/models/` (empty at its top); other roots as published.
pub(crate) fn cost_folder(path: Option<&str>) -> String {
    let Some(path) = path else {
        return OTHER_FOLDER.to_owned();
    };
    let directory: &str = path.rsplit_once('/').map_or("", |(directory, _)| directory);
    if directory == MODELS_ROOT {
        return String::new();
    }
    directory
        .strip_prefix(MODELS_ROOT)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(directory)
        .to_owned()
}

/// The cost rows of the nodes a selection picks over `period` and the one before, or why it's refused.
pub(crate) async fn selection_costs(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    ((select, exclude), period, default_role): ((&str, &str), &CostPeriod, DefaultRole),
) -> Result<SelectionCosts, ControlError> {
    let viewed: ViewedSelection = select_viewed_nodes(
        store,
        (planner, credentials),
        (location_id, target),
        ((select, exclude), default_role),
    )
    .await?;
    let mut result = SelectionCosts {
        snapshot_id: viewed.snapshot_id,
        error: viewed.error,
        rows: Vec::new(),
        pending_runs: BTreeSet::new(),
    };
    if result.error.is_some() {
        return Ok(result);
    }
    let selected: Option<BTreeSet<String>> = viewed.keys;
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, costs) = latest_costs_in(
        transaction,
        (location_id, target),
        period.previous_since,
        None,
    )
    .await?;
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for cost in &costs {
        let record = &cost.record;
        if record.resource_type == TOTAL_RESOURCE_TYPE {
            if PENDING_STATUSES.contains(&record.status.as_str()) {
                result.pending_runs.insert(record.run_id.clone());
            }
            continue;
        }
        let key: String = format!("{}:{}", record.resource_type, record.resource_name);
        if selected
            .as_ref()
            .is_none_or(|selected| selected.contains(&key))
        {
            keys.insert(key);
        }
    }
    let (transaction, nodes) = match &result.snapshot_id {
        Some(snapshot_id) => read_snapshot_nodes(transaction, *snapshot_id, Some(&keys)).await?,
        None => (transaction, BTreeMap::new()),
    };
    commit(transaction).await?;
    let folders: BTreeMap<&String, String> = keys
        .iter()
        .map(|key| {
            let path = nodes
                .get(key)
                .and_then(|node| node.definition["path"].as_str());
            (key, cost_folder(path))
        })
        .collect();
    for cost in costs {
        let record = cost.record;
        if record.resource_type == TOTAL_RESOURCE_TYPE {
            continue;
        }
        let node_key: String = format!("{}:{}", record.resource_type, record.resource_name);
        let Some(folder) = folders.get(&node_key) else {
            continue;
        };
        result.rows.push(CostRow {
            folder: folder.clone(),
            day: period.day(record.run_started_at),
            run_id: record.run_id,
            job: cost.job_key,
            credits: amount(&record.estimated_credits),
            usd: record.estimated_usd.as_deref().map(amount),
            seconds: amount(&record.attributed_seconds),
            queries: record.query_count,
            bytes_scanned: record.bytes_scanned,
            node_key,
        });
    }
    Ok(result)
}

/// A dollar total that stays unknown once any part of it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dollars(pub(crate) Option<f64>);

impl Default for Dollars {
    fn default() -> Self {
        Self(Some(0.0))
    }
}

impl Dollars {
    pub(crate) fn add(&mut self, usd: Option<f64>) {
        self.0 = self.0.zip(usd).map(|(total, usd)| total + usd);
    }
}
