//! Compile again with each set an unknown tag failed set back to its last good items.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::PoisonError;

use chrono::Utc;
use pyo3::PyResult;
use sqlbuild_orchestration::discovered_sets::main::record_attempt::record_attempt;
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSet;
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;

use crate::discovered_sets::_helpers::state::{FALLEN_BACK, SOURCE, TAG_FAILURES, USED};
use crate::discovered_sets::models::{DiscoverySource, LiveDiscovery, Rediscover};

/// `attempt`, made again while its compile failed sets on unknown tags that can fall back.
pub fn with_set_retries<T>(mut attempt: impl FnMut() -> PyResult<T>) -> PyResult<T> {
    let mut outcome: PyResult<T> = attempt();
    while outcome.is_err() && retry_without_failed_sets() {
        outcome = attempt();
    }
    outcome
}

/// Whether unknown tags failed sets, which now fall back to their last good items, then placeholders.
fn retry_without_failed_sets() -> bool {
    let failures: BTreeMap<String, String> =
        std::mem::take(&mut *TAG_FAILURES.lock().unwrap_or_else(PoisonError::into_inner));
    if failures.is_empty() {
        return false;
    }
    let mut source = SOURCE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(DiscoverySource::Live(live)) = source.as_ref() else {
        return false;
    };
    let Some(used) = USED.lock().unwrap_or_else(PoisonError::into_inner).clone() else {
        return false;
    };
    let mut fallen: std::sync::MutexGuard<'_, BTreeSet<String>> =
        FALLEN_BACK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut previous: DiscoveredSets = used.clone();
    for (name, error) in failures {
        let Some(current) = used.get(&name) else {
            continue;
        };
        let earlier: Option<&DiscoveredSet> = live.previous.get(&name);
        let mut failed: DiscoveredSet = record_attempt(
            earlier,
            Err(error),
            (Utc::now(), current.check_every_seconds),
        );
        let same_items: bool =
            earlier.and_then(|set| set.digest.as_ref()) == current.digest.as_ref();
        if !fallen.insert(name.clone()) || same_items {
            failed.result = None;
            failed.digest = None;
            failed.last_good_at = None;
        }
        previous.insert(name, failed);
    }
    let next: LiveDiscovery = LiveDiscovery {
        target: live.target.clone(),
        previous,
        rediscover: Rediscover::Only(BTreeSet::new()),
        keep_last_good: live.keep_last_good,
        seed: None,
    };
    *source = Some(DiscoverySource::Live(next));
    true
}
