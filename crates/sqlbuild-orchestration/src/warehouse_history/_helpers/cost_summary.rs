//! A selection's cost rows summed job by node, with daily series by job and top-level folder.

use std::collections::{BTreeMap, BTreeSet};

use crate::warehouse_history::_helpers::cost_selection::{
    CostPeriod, CostRow, Dollars, SelectionCosts,
};
use crate::warehouse_history::constants::CONFIGURED_PRICE;
use crate::warehouse_history::models::{
    CostFactRecord, CostPrice, CostSelectionRecord, CostSeriesGroup, CostSeriesRecord,
};

/// One job's cost on one node, as rows add up.
#[derive(Default)]
struct Fact {
    credits: f64,
    previous_credits: f64,
    usd: Dollars,
    previous_usd: Dollars,
    seconds: f64,
    runs: BTreeSet<String>,
    queries: i64,
    bytes_scanned: i64,
}

impl Fact {
    fn add(&mut self, row: &CostRow) {
        if row.day.is_none() {
            self.previous_credits += row.credits;
            self.previous_usd.add(row.usd);
            return;
        }
        self.credits += row.credits;
        self.usd.add(row.usd);
        self.seconds += row.seconds;
        self.runs.insert(row.run_id.clone());
        self.queries += row.queries;
        self.bytes_scanned += row.bytes_scanned;
    }

    fn record(self, (job, node_key, folder): (Option<String>, String, String)) -> CostFactRecord {
        CostFactRecord {
            job,
            node_key,
            folder,
            credits: self.credits,
            previous_credits: self.previous_credits,
            usd: self.usd.0,
            previous_usd: self.previous_usd.0,
            seconds: self.seconds,
            runs: count(self.runs.len()),
            queries: self.queries,
            bytes_scanned: self.bytes_scanned,
        }
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Credits per day of the period by `key`, for the period's rows.
fn daily<K: Ord>(
    period: &CostPeriod,
    rows: &[CostRow],
    key: impl Fn(&CostRow) -> K,
) -> BTreeMap<K, Vec<f64>> {
    let mut series: BTreeMap<K, Vec<f64>> = BTreeMap::new();
    for row in rows {
        if let Some(day) = row.day {
            series
                .entry(key(row))
                .or_insert_with(|| vec![0.0; period.days as usize])[day] += row.credits;
        }
    }
    series
}

/// The facts, most expensive first.
fn facts(rows: &[CostRow]) -> Vec<CostFactRecord> {
    let mut facts: BTreeMap<(Option<String>, String), (String, Fact)> = BTreeMap::new();
    for row in rows {
        facts
            .entry((row.job.clone(), row.node_key.clone()))
            .or_insert_with(|| (row.folder.clone(), Fact::default()))
            .1
            .add(row);
    }
    let mut records: Vec<CostFactRecord> = facts
        .into_iter()
        .map(|((job, node_key), (folder, fact))| fact.record((job, node_key, folder)))
        .collect();
    records.sort_by(|left, right| right.credits.total_cmp(&left.credits));
    records
}

/// The record of a selection's rows over `period`.
pub(crate) fn summarise(period: &CostPeriod, costs: SelectionCosts) -> CostSelectionRecord {
    let facts: Vec<CostFactRecord> = facts(&costs.rows);
    let (mut usd, mut previous_usd) = (Dollars::default(), Dollars::default());
    let mut runs: BTreeSet<&str> = BTreeSet::new();
    for row in &costs.rows {
        if row.day.is_some() {
            usd.add(row.usd);
            runs.insert(&row.run_id);
        } else {
            previous_usd.add(row.usd);
        }
    }
    let credits: f64 = facts.iter().map(|fact| fact.credits).sum();
    let usd: Option<f64> = usd.0.filter(|_| !runs.is_empty());
    let incomplete: usize = runs
        .iter()
        .filter(|run| costs.pending_runs.contains(**run))
        .count();
    let job_daily = daily(period, &costs.rows, |row| row.job.clone());
    let folder_daily = daily(period, &costs.rows, |row| {
        row.folder.split('/').next().unwrap_or_default().to_owned()
    });
    CostSelectionRecord {
        snapshot_id: costs.snapshot_id,
        since: period.since,
        days: period.days,
        dates: period.dates(),
        error: costs.error,
        price: usd.filter(|_| credits.is_normal()).map(|usd| CostPrice {
            usd_per_credit: usd / credits,
            source: CONFIGURED_PRICE.into(),
        }),
        credits,
        previous_credits: facts.iter().map(|fact| fact.previous_credits).sum(),
        usd,
        previous_usd: previous_usd.0,
        runs: count(runs.len()),
        incomplete_runs: count(incomplete),
        queries: facts.iter().map(|fact| fact.queries).sum(),
        seconds: facts.iter().map(|fact| fact.seconds).sum(),
        bytes_scanned: facts.iter().map(|fact| fact.bytes_scanned).sum(),
        job_daily: job_daily
            .into_iter()
            .map(|(key, credits)| CostSeriesRecord { key, credits })
            .collect(),
        folder_daily: folder_daily
            .into_iter()
            .map(|(key, credits)| CostSeriesRecord {
                key: Some(key),
                credits,
            })
            .collect(),
        facts,
    }
}

/// Each group's credits per day of the period, in the groups' order.
pub(crate) fn group_series(
    period: &CostPeriod,
    rows: &[CostRow],
    groups: &[CostSeriesGroup],
) -> Vec<Vec<f64>> {
    let mut series: Vec<Vec<f64>> = vec![vec![0.0; period.days as usize]; groups.len()];
    for row in rows {
        let Some(day) = row.day else {
            continue;
        };
        for (group, values) in groups.iter().zip(series.iter_mut()) {
            if in_group(group, row) {
                values[day] += row.credits;
            }
        }
    }
    series
}

/// Whether `row` is in `group`: every field the group gives matches.
fn in_group(group: &CostSeriesGroup, row: &CostRow) -> bool {
    let job: bool = if group.manual {
        row.job.is_none()
    } else {
        group
            .job
            .as_ref()
            .is_none_or(|job| row.job.as_ref() == Some(job))
    };
    let folder: bool = group.folder.as_ref().is_none_or(|folder| {
        folder.is_empty()
            || row.folder == *folder
            || row
                .folder
                .strip_prefix(folder.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    });
    let node: bool = group
        .node_key
        .as_ref()
        .is_none_or(|node_key| row.node_key == *node_key);
    job && folder && node
}
