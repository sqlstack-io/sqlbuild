//! Builders for `sqb debug` reports and stored runs, and plain text for what the session sends.

use serde_json::{Value, json};
use sqlbuild_core::screen_text::models::WarehouseState;
use sqlbuild_orchestration::runs::models::{RunRecord, RunStatus};
use uuid::Uuid;

use crate::dev_screen::models::ScreenMessage;
use crate::dev_session::_helpers::runs::SeenRuns;

/// A `sqb debug --json` report with these configuration and connection rows.
pub(super) fn report(configuration: Value, connection: Value) -> String {
    json!({
        "configuration": configuration,
        "connection": connection,
        "providers": [{"label": "providers", "message": "none discovered", "status": "OK"}],
    })
    .to_string()
}

/// One report row as `sqb debug --json` writes it; `status` is null for detail rows.
pub(super) fn row(label: &str, status: Option<&str>, message: &str, status_message: &str) -> Value {
    json!({"label": label, "status": status, "message": message, "status_message": status_message})
}

/// The state as `Connected { detail: "..." }`, `Failed { message: "..." }` and so on.
pub(super) fn state_text(state: &WarehouseState) -> String {
    format!("{state:?}")
}

/// `name | detail | running`, or `name | detail | ok 90s` / `failed 90s` once finished.
pub(super) fn message_text(message: &ScreenMessage) -> String {
    let ScreenMessage::Run {
        name,
        detail,
        finished,
    } = message
    else {
        return format!("{message:?}");
    };
    let outcome: String = finished.map_or_else(
        || "running".to_owned(),
        |(ok, elapsed)| {
            format!(
                "{} {}s",
                ["failed", "ok"][usize::from(ok)],
                elapsed.as_secs()
            )
        },
    );
    format!("{name} | {detail} | {outcome}")
}

/// The run with number `id`, named by `job_key` or `trigger_name`, that ran `seconds` with `nodes` nodes.
pub(super) fn run(
    id: u128,
    (status, origin): (&str, &str),
    (job_key, trigger_name): (Option<&str>, Option<&str>),
    (seconds, nodes): (i64, i64),
) -> RunRecord {
    let started = chrono::DateTime::parse_from_rfc3339("2026-11-15T09:40:00Z")
        .expect("start time")
        .to_utc();
    serde_json::from_value(json!({
        "run_id": Uuid::from_u128(id),
        "location_id": Uuid::from_u128(900),
        "target": "dev",
        "snapshot_id": Uuid::from_u128(901),
        "job_key": job_key,
        "kind": "adhoc",
        "origin": origin,
        "parent_run_id": null,
        "root_run_id": Uuid::from_u128(id),
        "trigger_id": null,
        "trigger_name": trigger_name,
        "tick_id": null,
        "rerun_of_run_id": null,
        "requested_by": "system:scheduler",
        "requester_name": null,
        "priority": 0,
        "rewrite_history": false,
        "status": status,
        "status_reason": null,
        "queued_at": started,
        "launched_at": started,
        "started_at": started,
        "ended_at": started + chrono::Duration::seconds(seconds),
        "cancel_requested_at": null,
        "selection_json": [],
        "node_counts_json": {"success": nodes - 1, "skipped": 1},
        "last_event_storage_id": 0,
    }))
    .expect("run record")
}

/// Runs already seen, by number and status.
pub(super) fn seen(runs: &[(u128, RunStatus)]) -> Option<SeenRuns> {
    Some(
        runs.iter()
            .map(|(id, status)| (Uuid::from_u128(*id), *status))
            .collect(),
    )
}
