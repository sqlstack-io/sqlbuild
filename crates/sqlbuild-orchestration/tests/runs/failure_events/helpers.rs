//! A claimed run's worker reports for `run_failed` tests: node results, an end-of-build audit that can't run, links under an origin.

use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::schema::models::Table;
use uuid::Uuid;

use crate::platform::models::RunFailureEvent;
use crate::runs::main::execution::change_run_linked::change_run_linked;
use crate::runs::main::execution::publish_run_recall::publish_run_recall;
use crate::runs::models::{
    NodeOutcome, RecallBatch, RecallRecord, RunAuthority, RunChange, RunChangeRequest, RunClaim,
};
use crate::runs::tests::test_types::{ReportedNode, RunFailureSummary};
use crate::store::models::ControlStore;

pub(crate) const ORIGIN: &str = "https://sqlbuild.example.com";
pub(crate) const MISSING: &str = "002003 (42S02): SQL compilation error: Object 'SHOP.RECON.ORDER_PAYMENTS' does not exist or not authorized.";

/// The worker reports `change` for the claimed run, linking a run failure under `ORIGIN`.
pub(crate) async fn report(store: &ControlStore, claim: &RunClaim, change: RunChange) {
    change_run_linked(
        store,
        (
            RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            },
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            },
        ),
        Some(ORIGIN),
    )
    .await
    .expect("report");
}

pub(crate) fn finished(node_key: &str, outcome: NodeOutcome, phase: Option<&str>) -> RunChange {
    RunChange::NodeFinished {
        node_key: node_key.into(),
        outcome,
        rows_affected: None,
        table_rows: None,
        no_change_reason: None,
        error_code: (outcome == NodeOutcome::Failed).then(|| "E100".into()),
        skipped_because_of: None,
        error_message: None,
        phase: phase.map(str::to_owned),
    }
}

/// An error-severity end-of-build singular audit whose query could not run.
pub(crate) async fn fail_end_audit(store: &ControlStore, claim: &RunClaim, audit: &str) {
    let payload = json!({"audit_name": audit, "audit_definition_name": audit,
        "evaluation_mode": "violations", "outcome": "error", "severity": "error", "run_scope_phase": "final",
        "attachment_kind": "end", "binding_key": format!("end.{audit}"), "executed_sql": "select 1",
        "execution_error": MISSING});
    let record = RecallRecord {
        table: Table::Events,
        values: BTreeMap::from([
            (
                "event_id".into(),
                RecallValue::Text(format!("audit-{audit}")),
            ),
            (
                "event_type".into(),
                RecallValue::Text("audit_completed".into()),
            ),
            ("event_schema_version".into(), RecallValue::Integer(1)),
            ("event_kind".into(), RecallValue::Text("lifecycle".into())),
            ("severity".into(), RecallValue::Text("error".into())),
            ("outcome".into(), RecallValue::Text("error".into())),
            (
                "occurred_at".into(),
                RecallValue::Text("2026-01-01T00:00:00Z".into()),
            ),
            ("producer".into(), RecallValue::Text("sqb".into())),
            ("run_id".into(), RecallValue::Text("sqb-run".into())),
            ("invocation_sequence".into(), RecallValue::Integer(0)),
            ("payload".into(), RecallValue::Text(payload.to_string())),
        ]),
    };
    publish_run_recall(
        store,
        (claim.request.run_id, claim.claim_id),
        &RecallBatch {
            invocation_id: "execution".into(),
            records: vec![record],
        },
    )
    .await
    .expect("publish the audit result");
}

/// The worker reports `nodes` finishing, then `failed_end_audits` that couldn't run, then a failed finish.
pub(crate) async fn end_run(
    store: &ControlStore,
    claim: &RunClaim,
    (nodes, failed_end_audits): (&[ReportedNode], &[&str]),
) {
    for (node, outcome, phase) in nodes {
        report(store, claim, finished(node, *outcome, *phase)).await;
    }
    for audit in failed_end_audits {
        fail_end_audit(store, claim, audit).await;
    }
    report(
        store,
        claim,
        RunChange::Finished {
            outcome: crate::runs::models::RunOutcome::Failed,
            reason: "project could not be loaded".into(),
            exit_code: Some(2),
        },
    )
    .await;
}

/// What a test compares of a `run_failed` event, with links relative to the run's own page.
pub(crate) fn summarize(failure: &RunFailureEvent) -> RunFailureSummary {
    let run_url = format!("{ORIGIN}/runs/{}", failure.run_id);
    let relative = |link: &Option<String>| {
        link.as_deref()
            .and_then(|link| link.strip_prefix(run_url.as_str()))
            .map(str::to_owned)
    };
    RunFailureSummary {
        links_to_run: failure.url.as_deref() == Some(run_url.as_str()),
        first_node: failure.failed_nodes.first().map(|node| {
            (
                node.phase.clone(),
                relative(&node.url),
                relative(&node.logs_url),
            )
        }),
        first_audit: failure.failed_audits.first().map(|audit| {
            (
                audit.name.clone(),
                audit.sqlstate.clone(),
                audit.message.clone(),
            )
        }),
        failed_audit_count: failure.failed_audit_count,
        succeeded_node_count: failure.succeeded_node_count,
        error: failure
            .error
            .as_ref()
            .map(|error| (error.phase, error.message.clone())),
    }
}
