//! The location's last finished plans, in memory, returned again for an identical request.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};
use sqlbuild_orchestration::platform::models::PlanModelDiff;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::constants::{REUSED_PLAN_AGE, REUSED_PLAN_BYTES, REUSED_PLANS};
use crate::native_commands::models::{PlanKey, PlanReuse, PlanToKeep, ReusedPlan};
use crate::native_commands::types::LocationPlan;

/// The location's kept plans; one location serves one target.
pub(crate) static PLAN_REUSE: Mutex<Option<PlanReuse>> = Mutex::new(None);

/// The kept plan for `key` under this generation and copy, if younger than the reuse age at `now`.
pub(crate) fn reused_plan(
    reuse: &Mutex<Option<PlanReuse>>,
    (generation, version): (i64, &str),
    key: &PlanKey,
    now: DateTime<Utc>,
) -> Option<LocationPlan> {
    let kept: Arc<ReusedPlan> = {
        let mut reuse = reuse.lock().unwrap_or_else(PoisonError::into_inner);
        let current: &mut PlanReuse = reuse
            .as_mut()
            .filter(|reuse| reuse.generation == generation && reuse.version == version)?;
        while current
            .plans
            .front()
            .is_some_and(|plan| now - plan.planned_at >= REUSED_PLAN_AGE)
        {
            if let Some(expired) = current.plans.pop_front() {
                current.bytes -= plan_bytes(&expired);
            }
        }
        Arc::clone(current.plans.iter().find(|plan| plan.key == *key)?)
    };
    let read = serde_json::from_str::<Value>(&kept.document).and_then(|document| {
        serde_json::from_str::<BTreeMap<String, PlanModelDiff>>(&kept.diffs)
            .map(|diffs| (document, diffs))
    });
    let (mut document, diffs) = match read {
        Ok(read) => read,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("A kept plan could not be read, so the location plans again: {error}\n"),
            )]));
            return None;
        }
    };
    document["timings"]["reused_from"] =
        json!(kept.planned_at.to_rfc3339_opts(SecondsFormat::Secs, true));
    Some((document, diffs))
}

/// Keep a checked, successful plan within `(plans, bytes)`, oldest out first; a newer generation or copy replaces all.
pub(crate) fn remember_plan(
    (reuse, (most_plans, most_bytes)): (&Mutex<Option<PlanReuse>>, (usize, usize)),
    keep: PlanToKeep,
    (document, diffs): (&Value, &BTreeMap<String, PlanModelDiff>),
) {
    if !document["error"].is_null() {
        return;
    }
    let (Ok(text), Ok(diff_text)) = (
        serde_json::to_string(document),
        serde_json::to_string(diffs),
    ) else {
        return;
    };
    let kept = Arc::new(ReusedPlan {
        key: keep.key,
        document: text,
        diffs: diff_text,
        planned_at: keep.planned_at,
    });
    if plan_bytes(&kept) > most_bytes {
        return;
    }
    let mut reuse = reuse.lock().unwrap_or_else(PoisonError::into_inner);
    let current: &mut PlanReuse = match reuse.as_mut() {
        Some(current)
            if current.generation == keep.generation && current.version == keep.version =>
        {
            current
        }
        _ => reuse.insert(PlanReuse {
            generation: keep.generation,
            version: keep.version,
            plans: VecDeque::new(),
            bytes: 0,
        }),
    };
    let replaced: usize = current
        .plans
        .iter()
        .filter(|plan| plan.key == kept.key)
        .map(|plan| plan_bytes(plan))
        .sum();
    current.plans.retain(|plan| plan.key != kept.key);
    current.bytes = current.bytes - replaced + plan_bytes(&kept);
    current.plans.push_back(kept);
    while current.plans.len() > most_plans || current.bytes > most_bytes {
        if let Some(dropped) = current.plans.pop_front() {
            current.bytes -= plan_bytes(&dropped);
        }
    }
}

/// Keep a plan for identical requests once the checks after planning passed.
pub(crate) fn keep_plan(
    keep: Option<PlanToKeep>,
    plan: (&Value, &BTreeMap<String, PlanModelDiff>),
) {
    if let Some(keep) = keep {
        remember_plan((&PLAN_REUSE, (REUSED_PLANS, REUSED_PLAN_BYTES)), keep, plan);
    }
}

/// The bytes a kept plan's text holds.
fn plan_bytes(plan: &ReusedPlan) -> usize {
    plan.document.len() + plan.diffs.len()
}
