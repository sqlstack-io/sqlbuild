//! One run's Recall facts for its `run_failed` event: error-severity audit failures, the failing invocation's error and reported phases.

use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use uuid::Uuid;

use crate::runs::main::listings::run_invocations::run_invocations;
use crate::runs::models::RunAuditFailure;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::ControlTransaction;
use crate::warehouse_history::_helpers::run_recall::{RunRecallScope, invocation_filter, sqlstate};
use crate::warehouse_history::constants::INVOCATION_PHASE_EVENT;
use crate::warehouse_history::models::RunFailureRecall;

/// One run's error-severity audit failures, latest invocation error and reported phases.
pub(crate) async fn read_member_failure_recall(
    mut transaction: ControlTransaction,
    member: Uuid,
) -> Result<(ControlTransaction, RunFailureRecall), ControlError> {
    let mut recall = RunFailureRecall::default();
    let namespaces;
    (transaction, namespaces) = run_invocations(transaction, member).await?;
    let scope = RunRecallScope::of_invocations(namespaces);
    let (filter, parameters) = invocation_filter(&scope, 1);
    let audits;
    (transaction, audits) = query(
        transaction,
        &format!(
            "SELECT audit_name, attached_target_kind, attached_target_name, violation_count, execution_error \
             FROM audit_results WHERE {filter} AND outcome='error' AND ignored=0 ORDER BY audit_name"
        ),
        parameters.clone(),
    )
    .await?;
    for row in audits {
        recall.audit_count += 1;
        let text = |column: &str| row[column].as_str().map(str::to_owned);
        let message = text("execution_error").map(|error| redact_error_text(&error));
        recall.audits.push(RunAuditFailure {
            name: text("audit_name").unwrap_or_default(),
            node_key: text("attached_target_kind")
                .zip(text("attached_target_name"))
                .map(|(kind, name)| format!("{kind}:{name}")),
            sqlstate: message.as_deref().and_then(sqlstate),
            message,
            failing_rows: row["violation_count"].as_i64(),
        });
    }
    let errors;
    (transaction, errors) = query(
        transaction,
        &format!(
            "SELECT error_type, error_message FROM invocations \
             WHERE {filter} AND error_message IS NOT NULL ORDER BY ended_at DESC LIMIT 1"
        ),
        parameters.clone(),
    )
    .await?;
    if let Some(row) = errors.first() {
        recall.invocation_error = Some((
            row["error_type"].as_str().map(str::to_owned),
            row["error_message"].as_str().map(redact_error_text),
        ));
    }
    let phases;
    (transaction, phases) = query(
        transaction,
        &format!(
            "SELECT DISTINCT payload->>'phase' AS phase FROM recall_events \
             WHERE {filter} AND event_type='{INVOCATION_PHASE_EVENT}'"
        ),
        parameters,
    )
    .await?;
    recall.phases.extend(
        phases
            .iter()
            .filter_map(|row| row["phase"].as_str().map(str::to_owned)),
    );
    Ok((transaction, recall))
}
