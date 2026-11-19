//! The location's last finished plans, in memory, returned again for an identical request.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::constants::REUSED_PLANS;
use crate::native_commands::models::{PlanKey, PlanReuse, ReusedPlan};
use crate::native_commands::types::LocationPlan;

/// The location's kept plans; one location serves one target.
pub(crate) static PLAN_REUSE: Mutex<Option<PlanReuse>> = Mutex::new(None);

/// The kept plan for `key` under this generation and copy, marked as reused.
pub(crate) fn reused_plan(
    reuse: &Mutex<Option<PlanReuse>>,
    (generation, version): (i64, &str),
    key: &PlanKey,
) -> Option<LocationPlan> {
    let kept: ReusedPlan = reuse
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .filter(|reuse| reuse.generation == generation && reuse.version == version)?
        .plans
        .iter()
        .find(|plan| plan.key == *key)?
        .clone();
    let mut document: Value = match serde_json::from_str(&kept.document) {
        Ok(document) => document,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("A kept plan could not be read, so the location plans again: {error}\n"),
            )]));
            return None;
        }
    };
    let planned_from: Value = document["timings"]["planned_from"].clone();
    document["timings"] = json!({
        "planned_from": planned_from,
        "reused_from": kept.planned_at,
        "total_seconds": 0.0,
        "steps": [{"name": format!("Reused plan from {}", kept.planned_at), "seconds": 0.0}],
    });
    Some((document, kept.diffs))
}

/// Keep a finished plan for `key`, unless it failed; a newer generation or copy replaces all.
pub(crate) fn remember_plan(
    reuse: &Mutex<Option<PlanReuse>>,
    (generation, version): (i64, &str),
    key: PlanKey,
    ((document, diffs), planned_at): (&LocationPlan, DateTime<Utc>),
) {
    if !document["error"].is_null() {
        return;
    }
    let Ok(text) = serde_json::to_string(document) else {
        return;
    };
    let mut reuse = reuse.lock().unwrap_or_else(PoisonError::into_inner);
    let current: &mut PlanReuse = match reuse.as_mut() {
        Some(current) if current.generation == generation && current.version == version => current,
        _ => reuse.insert(PlanReuse {
            generation,
            version: version.to_owned(),
            plans: VecDeque::new(),
        }),
    };
    current.plans.retain(|plan| plan.key != key);
    current.plans.push_back(ReusedPlan {
        key,
        document: text,
        diffs: diffs.clone(),
        planned_at: planned_at.to_rfc3339_opts(SecondsFormat::Secs, true),
    });
    while current.plans.len() > REUSED_PLANS {
        current.plans.pop_front();
    }
}
