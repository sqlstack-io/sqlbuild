//! Queue a failed run's next `[retry]` attempt in the same transaction, inside its group run if any.

use std::collections::BTreeSet;

use chrono::TimeDelta;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::declarations::models::OnFailure;
use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::_helpers::requests::rerun::frozen_rerun;
use crate::runs::constants::NODE_FAILED;
use crate::runs::constants::{MEMBER_RECOVERED_EVENT, MEMBER_RETRYING_EVENT};
use crate::runs::main::requests::queue_run::queue_run;
use crate::runs::models::{RerunRequest, RunCommand, RunOrigin, RunRequest, RunRetry};
use crate::snapshots::constants::SOURCE_KEY_PREFIX;
use crate::snapshots::main::write_sets::job_write_set::job_write_set;
use crate::store::constants::UNREADABLE_SNAPSHOT;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Whether a failed run's retry was queued, and whether it stays inside a group run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueuedRetry {
    None,
    Run,
    Member,
}

/// Rerun the failed and skipped nodes after the policy's delay, unless the run was cancelled, used up its retries, or its retry's ID is already taken.
pub(crate) async fn queue_retry(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, QueuedRetry), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT r.request_json, r.cancel_requested_at, \
         (SELECT COUNT(*) FROM sqb_run_nodes n WHERE n.run_id=r.run_id AND n.kind='node' AND n.status IN ('failed','skipped','not_run')) AS retryable \
         FROM sqb_runs r WHERE r.run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((transaction, QueuedRetry::None));
    };
    let source: RunRequest = serde_json::from_value(row["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let retryable = row["retryable"].as_i64().unwrap_or_default() > 0;
    let Some(job_key) = source
        .job_key
        .clone()
        .filter(|_| row["cancel_requested_at"].is_null() && retryable)
    else {
        return Ok((transaction, QueuedRetry::None));
    };
    let (transaction, definitions) = frozen_definitions(transaction, source.snapshot_id).await?;
    let Ok(definitions) = definitions else {
        return Ok((not_retried(transaction, run_id).await?, QueuedRetry::None));
    };
    if source.parent_run_id.is_some() {
        let policy = definitions
            .jobs
            .get(&job_key)
            .and_then(|job| job.on_failure);
        return queue_member_retry(transaction, &source, policy).await;
    }
    let policy = definitions
        .jobs
        .get(&job_key)
        .or_else(|| definitions.groups.get(&job_key).map(|group| &group.job))
        .and_then(|job| job.on_failure);
    let attempt = source
        .retry
        .map_or(1, |retry| retry.attempt.saturating_add(1));
    let Some(policy) = policy.filter(|policy| attempt <= policy.retries) else {
        return Ok((transaction, QueuedRetry::None));
    };
    let retry_id = Uuid::new_v5(&run_id, b"retry");
    let (transaction, taken) = query(
        transaction,
        "SELECT 1 FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(retry_id)],
    )
    .await?;
    if !taken.is_empty() {
        return Ok((transaction, QueuedRetry::None));
    }
    let context = AuditContext {
        actor: "system:retry".into(),
        granting_role: "system:scheduler".into(),
        request_id: retry_id,
        ip_address: None,
    };
    let request = RerunRequest {
        run_id: retry_id,
        source_run_id: run_id,
        from_failure: true,
        reason: format!("retry {attempt} of {}", policy.retries),
        priority: source.priority,
        nodes: Vec::new(),
    };
    let (transaction, (_, mut frozen)) =
        frozen_rerun(transaction, (&context, None), &request).await?;
    frozen.retry = Some(RunRetry {
        attempt,
        not_before: transaction.now + TimeDelta::seconds(i64::from(policy.retry_delay_seconds)),
    });
    Ok((queue_run(transaction, &frozen).await?, QueuedRetry::Run))
}

/// The failed run's status reason also says it was not retried because its snapshot can't be read, unless that is already why it failed.
async fn not_retried(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_runs SET status_reason=CASE WHEN status_reason IS NULL OR status_reason='' THEN $2 \
         ELSE status_reason || '; not retried: ' || $2 END \
         WHERE run_id=$1 AND (status_reason IS NULL OR status_reason<>$2)",
        vec![
            Parameter::Id(run_id),
            Parameter::Text(UNREADABLE_SNAPSHOT.into()),
        ],
    )
    .await?;
    Ok(transaction)
}

/// The member's next attempt inside its running group: its failed nodes and those it skipped because of them.
async fn queue_member_retry(
    transaction: ControlTransaction,
    source: &RunRequest,
    policy: Option<OnFailure>,
) -> Result<(ControlTransaction, QueuedRetry), ControlError> {
    let Some(parent_id) = source.parent_run_id else {
        return Ok((transaction, QueuedRetry::None));
    };
    let attempt = source.member_attempt.unwrap_or(1).saturating_add(1);
    let Some(policy) = policy.filter(|policy| attempt <= policy.retries.saturating_add(1)) else {
        return Ok((transaction, QueuedRetry::None));
    };
    let (transaction, parent) = query(
        transaction,
        "SELECT 1 FROM sqb_runs WHERE run_id=$1 AND status='running' AND cancel_requested_at IS NULL",
        vec![Parameter::Id(parent_id)],
    )
    .await?;
    let retry_id = Uuid::new_v5(&source.run_id, b"retry");
    let (transaction, taken) = query(
        transaction,
        "SELECT 1 FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(retry_id)],
    )
    .await?;
    if parent.is_empty() || !taken.is_empty() {
        return Ok((transaction, QueuedRetry::None));
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT node_key, status, skipped_because_of FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' \
         AND status IN ('failed','skipped','not_run')",
        vec![Parameter::Id(source.run_id)],
    )
    .await?;
    let selection: BTreeSet<String> = rows
        .iter()
        .filter(|row| {
            row["status"] == NODE_FAILED
                || row["skipped_because_of"]
                    .as_str()
                    .is_none_or(|cause| source.selection.contains(cause))
        })
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    if selection.is_empty() {
        return Ok((transaction, QueuedRetry::None));
    }
    let (transaction, writers) = job_write_set(
        transaction,
        source.snapshot_id,
        (&selection, &source.settings),
    )
    .await?;
    let mut write_set: BTreeSet<String> = source
        .write_set
        .iter()
        .filter(|key| writers.contains(*key))
        .cloned()
        .collect();
    if source.command == RunCommand::Build {
        write_set.extend(
            writers
                .into_iter()
                .filter(|key| key.starts_with(SOURCE_KEY_PREFIX)),
        );
    }
    let not_before = transaction.now + TimeDelta::seconds(i64::from(policy.retry_delay_seconds));
    let request = RunRequest {
        run_id: retry_id,
        origin: RunOrigin::Rerun,
        rerun_of_run_id: Some(source.run_id),
        requested_by: "system:retry".into(),
        selection,
        write_set,
        retry: Some(RunRetry {
            attempt: attempt - 1,
            not_before,
        }),
        member_attempt: Some(attempt),
        ..source.clone()
    };
    let transaction = queue_run(transaction, &request).await?;
    let job_key = source.job_key.clone().unwrap_or_default();
    let (transaction, _) = append_run_event(
        transaction,
        (
            parent_id,
            Uuid::new_v5(&retry_id, MEMBER_RETRYING_EVENT.as_bytes()),
        ),
        MEMBER_RETRYING_EVENT,
        json!({
            "resource_kind": "job",
            "resource_name": job_key,
            "member": {
                "job_key": job_key,
                "run_id": source.run_id,
                "retry_run_id": retry_id,
                "attempt": attempt,
                "attempts": policy.retries.saturating_add(1),
                "delay_seconds": policy.retry_delay_seconds,
                "not_before": not_before,
            }
        }),
    )
    .await?;
    Ok((transaction, QueuedRetry::Member))
}

/// Record on its group run that a retried member succeeded, and on which attempt.
pub(crate) async fn record_member_recovery(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT request_json FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(transaction);
    };
    let request: RunRequest = serde_json::from_value(row["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (Some(parent_id), Some(attempt)) = (request.parent_run_id, request.member_attempt) else {
        return Ok(transaction);
    };
    let job_key = request.job_key.unwrap_or_default();
    let (transaction, _) = append_run_event(
        transaction,
        (
            parent_id,
            Uuid::new_v5(&run_id, MEMBER_RECOVERED_EVENT.as_bytes()),
        ),
        MEMBER_RECOVERED_EVENT,
        json!({
            "resource_kind": "job",
            "resource_name": job_key,
            "member": {"job_key": job_key, "run_id": run_id, "attempt": attempt}
        }),
    )
    .await?;
    Ok(transaction)
}
