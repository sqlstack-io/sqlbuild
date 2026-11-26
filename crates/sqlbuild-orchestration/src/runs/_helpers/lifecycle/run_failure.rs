//! A failed run becomes one platform event saying what failed, where and why, with links to the UI.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;

use crate::platform::constants::RUN_FAILURE_NODE_LIMIT;
use crate::platform::models::{PlatformEvent, RunFailureEvent};
use crate::runs::_helpers::lifecycle::error_text::finished_failure;
use crate::runs::models::{
    RunChangeRequest, RunFailureCause, RunFailurePhase, RunNodeFailure, RunOrigin, RunOutcome,
    RunTrigger, RunTriggerKind,
};
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};
use crate::store::types::Row;
use crate::warehouse_history::main::runs::read_run_failure_recall::read_run_failure_recall;

/// The worker phase that ends loading the project, and the one that starts building.
const COMPILED_PHASE: &str = "compiled";
const EXECUTION_PHASE: &str = "started_execution";

/// Runs after the terminal projection, so nodes left running already read as failed.
pub(crate) async fn publish_run_failure(
    transaction: ControlTransaction,
    (request, outcome): (&RunChangeRequest, RunOutcome),
    origin: Option<&str>,
) -> Result<ControlTransaction, ControlError> {
    let invalid = |error: serde_json::Error| ControlError::Invalid(error.to_string());
    let (transaction, runs) = query(
        transaction,
        "SELECT r.kind, r.job_key, r.location_id, r.target, r.snapshot_id, r.origin, t.name AS trigger_name, \
         p.name AS requester_name, s.principal_id AS service_account, r.status_reason, r.launched_at, r.started_at, r.ended_at \
         FROM sqb_runs r LEFT JOIN sqb_triggers t ON t.trigger_id=r.trigger_id \
         LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=r.requested_by \
         LEFT JOIN sqb_service_accounts s ON s.principal_id=p.principal_id WHERE r.run_id=$1",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let run = runs
        .into_iter()
        .next()
        .ok_or_else(|| ControlError::Invalid("finished run is missing".into()))?;
    let (transaction, counts) = query(
        transaction,
        "SELECT status, COUNT(*) AS count FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' GROUP BY status",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let counted = |statuses: &[&str]| -> i64 {
        counts
            .iter()
            .filter(|row| statuses.contains(&row["status"].as_str().unwrap_or_default()))
            .filter_map(|row| row["count"].as_i64())
            .sum()
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT n.node_key, n.error_code, e.payload FROM sqb_run_nodes n \
         LEFT JOIN recall_events e ON e.storage_id=n.finished_storage_id \
         WHERE n.run_id=$1 AND n.kind='node' AND n.status='failed' ORDER BY n.node_key LIMIT $2",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Integer(RUN_FAILURE_NODE_LIMIT),
        ],
    )
    .await?;
    let (transaction, skipped) = query(
        transaction,
        "SELECT node_key FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' AND status='skipped' \
         ORDER BY node_key LIMIT $2",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Integer(RUN_FAILURE_NODE_LIMIT),
        ],
    )
    .await?;
    let (transaction, recall) = read_run_failure_recall(
        transaction,
        (
            request.run_id,
            usize::try_from(RUN_FAILURE_NODE_LIMIT).unwrap_or_default(),
        ),
    )
    .await?;
    let keys: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    let snapshot_id = serde_json::from_value(run["snapshot_id"].clone()).map_err(invalid)?;
    let (mut transaction, nodes) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&keys)).await?;
    let links = Links {
        origin,
        run_id: request.run_id,
    };
    let mut failed_nodes: Vec<RunNodeFailure> = Vec::new();
    for row in rows {
        let node_key = row["node_key"].as_str().unwrap_or_default().to_owned();
        let (message, phase) = finished_failure(&row["payload"])?;
        failed_nodes.push(RunNodeFailure {
            error_code: row["error_code"].as_str().map(str::to_owned),
            message,
            path: nodes
                .get(&node_key)
                .and_then(|node| node.definition.get("path"))
                .and_then(Value::as_str)
                .map(|path| path.replace('\\', "/")),
            phase,
            url: links.node(&node_key, false),
            logs_url: links.node(&node_key, true),
            node_key,
        });
    }
    let failed_node_count = counted(&["failed"]);
    let time = |column: &str| -> Result<Option<DateTime<Utc>>, ControlError> {
        serde_json::from_value(run[column].clone()).map_err(invalid)
    };
    let (started_at, ended_at) = (time("started_at")?, time("ended_at")?);
    let error = (failed_node_count == 0 && recall.audit_count == 0).then(|| {
        let (code, message) = recall.invocation_error.clone().unwrap_or_default();
        RunFailureCause {
            phase: failure_phase(
                (run["launched_at"].is_null(), started_at.is_none()),
                &recall.phases,
            ),
            code,
            message: message.or_else(|| run["status_reason"].as_str().map(redact_error_text)),
        }
    });
    let platform = PlatformEvent::RunFailed(Box::new(RunFailureEvent {
        run_id: request.run_id,
        run_kind: serde_json::from_value(run["kind"].clone()).map_err(invalid)?,
        job_key: run["job_key"].as_str().map(str::to_owned),
        location_id: serde_json::from_value(run["location_id"].clone()).map_err(invalid)?,
        target: run["target"].as_str().unwrap_or_default().to_owned(),
        outcome,
        url: links.run(),
        trigger: trigger(&run).map_err(invalid)?,
        started_at,
        ended_at,
        duration_ms: started_at
            .zip(ended_at)
            .map(|(start, end)| (end - start).num_milliseconds()),
        error,
        failed_nodes,
        failed_node_count,
        failed_audits: recall.audits,
        failed_audit_count: recall.audit_count,
        succeeded_node_count: counted(&["success", "warn"]),
        skipped_nodes: skipped
            .iter()
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect(),
        skipped_node_count: counted(&["skipped"]),
    }));
    let occurred_at = transaction.now;
    let appended;
    (transaction, appended) = append_control_event(
        transaction,
        ControlEvent {
            event_id: uuid::Uuid::new_v5(&request.event_id, b"platform-run-failed"),
            invocation_id: format!("platform/{}", request.run_id),
            event_type: platform.event_type(),
            producer: "platform",
            occurred_at,
            run_id: Some(request.run_id),
            payload: json!({"platform": platform}),
        },
    )
    .await?;
    if appended.conflict.is_some() {
        return Err(ControlError::Invalid(
            "run failure conflicts with a durable fact".into(),
        ));
    }
    Ok(transaction)
}

/// UI links under the server's origin, as sensor events build theirs; none without one.
struct Links<'a> {
    origin: Option<&'a str>,
    run_id: uuid::Uuid,
}

impl Links<'_> {
    fn run(&self) -> Option<String> {
        self.origin
            .map(|origin| format!("{}/runs/{}", origin.trim_end_matches('/'), self.run_id))
    }

    /// The node on the run's page (`?node=`), or its output (`/output/<node>`).
    fn node(&self, node_key: &str, logs: bool) -> Option<String> {
        let key = percent_encoded(node_key);
        self.run().map(|run| {
            if logs {
                format!("{run}/output/{key}")
            } else {
                format!("{run}?node={key}")
            }
        })
    }
}

/// `text` with everything but unreserved URL characters percent-encoded.
fn percent_encoded(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// Where a run failed outside any node: before launch, before its worker started, or after its last reported phase.
fn failure_phase(
    (never_launched, never_started): (bool, bool),
    phases: &BTreeSet<String>,
) -> RunFailurePhase {
    if never_launched {
        RunFailurePhase::Launch
    } else if never_started {
        RunFailurePhase::WorkerStart
    } else if phases.contains(EXECUTION_PHASE) {
        RunFailurePhase::Build
    } else if phases.contains(COMPILED_PHASE) {
        RunFailurePhase::Planning
    } else {
        RunFailurePhase::Load
    }
}

/// What started the run: its schedule or sensor by name, else the person or service account who asked.
fn trigger(run: &Row) -> Result<RunTrigger, serde_json::Error> {
    let origin: RunOrigin = serde_json::from_value(run["origin"].clone())?;
    let name = run["trigger_name"].as_str().map(str::to_owned);
    let person = run["requester_name"].as_str().map(str::to_owned);
    Ok(match origin {
        RunOrigin::Schedule => RunTrigger {
            kind: RunTriggerKind::Schedule,
            name,
            person: None,
        },
        RunOrigin::Sensor => RunTrigger {
            kind: RunTriggerKind::Sensor,
            name,
            person: None,
        },
        RunOrigin::Manual | RunOrigin::Rerun | RunOrigin::Backfill => RunTrigger {
            kind: if run["service_account"].is_null() {
                RunTriggerKind::Manual
            } else {
                RunTriggerKind::Api
            },
            name: None,
            person,
        },
    })
}
