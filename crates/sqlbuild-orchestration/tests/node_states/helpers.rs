//! Node states for status tests: builds and runs at hours after one fixed start.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::node_states::_helpers::status::{StatusNode, apply_statuses};
use crate::node_states::models::NodeState;
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeOutcome};

pub(crate) fn at(hours: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 0, 0, 0)
        .single()
        .expect("a valid fixture start")
        + Duration::hours(hours)
}

pub(crate) fn built(hours: i64) -> Option<CatalogNodeBuild> {
    Some(CatalogNodeBuild {
        observed_at: at(hours),
        run_id: "run".to_owned(),
        version_hash: "hash".to_owned(),
    })
}

pub(crate) fn outcome(status: &str, hours: i64) -> Option<CatalogNodeOutcome> {
    Some(CatalogNodeOutcome {
        status: status.to_owned(),
        at: at(hours),
        run_id: None,
    })
}

pub(crate) fn state(
    last_build: Option<CatalogNodeBuild>,
    last_outcome: Option<CatalogNodeOutcome>,
    changed: bool,
) -> NodeState {
    NodeState {
        last_build,
        last_outcome,
        definition_changed: changed,
        ..NodeState::default()
    }
}

/// A small project: tables, views and nodes without a table, with builds and runs at different hours.
pub(crate) fn project_statuses() -> BTreeMap<String, NodeState> {
    let node = |key, kind, has_table, view, upstream: &[&'static str]| StatusNode {
        key,
        kind,
        has_table,
        view,
        upstream: upstream.to_vec(),
    };
    let nodes = [
        node("source:raw", "source", false, false, &[]),
        node("model:fresh", "model", true, false, &[]),
        node("model:old", "model", true, false, &["model:fresh"]),
        node("model:view", "model", true, true, &["model:fresh"]),
        node("model:over_view", "model", true, false, &["model:view"]),
        node("model:edited", "model", true, false, &[]),
        node("model:edited_failed", "model", true, false, &[]),
        node("model:unbuilt", "model", true, false, &[]),
        node("model:unbuilt_failed", "model", true, false, &[]),
        node("asset:ran", "asset", false, false, &["model:fresh"]),
        node("asset:warned", "asset", false, false, &[]),
        node("asset:failed", "asset", false, false, &[]),
        node("asset:never", "asset", false, false, &[]),
        node("model:loop_a", "model", true, true, &["model:loop_b"]),
        node("model:loop_b", "model", true, true, &["model:loop_a"]),
    ];
    let states: BTreeMap<String, NodeState> = BTreeMap::from(
        [
            ("source:raw", state(None, outcome("success", 1), false)),
            ("model:fresh", state(built(5), outcome("success", 5), false)),
            ("model:old", state(built(2), outcome("success", 2), false)),
            ("model:view", state(built(1), outcome("success", 1), false)),
            (
                "model:over_view",
                state(built(3), outcome("success", 3), false),
            ),
            ("model:edited", state(built(4), outcome("success", 4), true)),
            (
                "model:edited_failed",
                state(built(4), outcome("failed", 6), true),
            ),
            ("model:unbuilt", state(None, None, false)),
            (
                "model:unbuilt_failed",
                state(None, outcome("failed", 6), false),
            ),
            ("asset:ran", state(None, outcome("success", 7), false)),
            ("asset:warned", state(None, outcome("warn", 8), false)),
            ("asset:failed", state(None, outcome("failed", 9), false)),
            ("asset:never", state(None, None, false)),
            ("model:loop_a", state(built(1), None, false)),
            ("model:loop_b", state(built(1), None, false)),
        ]
        .map(|(key, state)| (key.to_owned(), state)),
    );
    apply_statuses(&nodes, states)
}
