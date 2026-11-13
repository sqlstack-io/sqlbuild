//! A failed or partial top-level run of a job with `on_failure` queues its next retry in the same transaction.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::_helpers::requests::rerun::frozen_rerun;
use crate::runs::main::queue_run::queue_run;
use crate::runs::models::{RerunRequest, RunRequest, RunRetry};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Rerun the failed and skipped nodes after the policy's delay, unless the run was cancelled, is a group member, or used up its retries.
pub(crate) async fn queue_retry(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT r.request_json, r.cancel_requested_at, \
         (SELECT COUNT(*) FROM sqb_run_nodes n WHERE n.run_id=r.run_id AND n.kind='node' AND n.status IN ('failed','skipped')) AS retryable \
         FROM sqb_runs r WHERE r.run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(transaction);
    };
    let source: RunRequest = serde_json::from_value(row["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let retryable = row["retryable"].as_i64().unwrap_or_default() > 0;
    let Some(job_key) = source.job_key.as_deref().filter(|_| {
        source.parent_run_id.is_none() && row["cancel_requested_at"].is_null() && retryable
    }) else {
        return Ok(transaction);
    };
    let (transaction, definitions) = frozen_definitions(transaction, source.snapshot_id).await?;
    let policy = definitions
        .jobs
        .get(job_key)
        .or_else(|| definitions.groups.get(job_key).map(|group| &group.job))
        .and_then(|job| job.on_failure);
    let attempt = source
        .retry
        .map_or(1, |retry| retry.attempt.saturating_add(1));
    let Some(policy) = policy.filter(|policy| attempt <= policy.retries) else {
        return Ok(transaction);
    };
    let retry_id = Uuid::new_v5(&run_id, b"retry");
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
    queue_run(transaction, &frozen).await
}
