//! Lifecycle authority checks precede fact publication and physical claim release.

use crate::runs::models::{
    ChangingRun, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// A launch-response observation is append-only even after completion, but requires recorded ownership.
pub(crate) async fn check_launch_claim(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
) -> Result<ControlTransaction, ControlError> {
    let claim_id = request.claim_id.ok_or_else(|| {
        ControlError::Invalid("launch observation requires a recorded claim".into())
    })?;
    let (transaction, rows) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE event_id=$1 AND run_id=$2 \
         AND event_type='run_claimed' AND producer='scheduler' AND invocation_id=$3",
        vec![
            Parameter::Text(claim_id.to_string()),
            Parameter::Text(request.run_id.to_string()),
            Parameter::Text(format!("scheduler/{}", request.run_id)),
        ],
    )
    .await?;
    let recorded = rows.first().map(|row| &row["payload"]["claim"]);
    if recorded.is_none_or(|claim| {
        claim["claim_id"] != claim_id.to_string()
            || claim["request"]["run_id"] != request.run_id.to_string()
    }) {
        return Err(ControlError::Invalid(
            "launch observation does not belong to a recorded claim".into(),
        ));
    }
    Ok(transaction)
}

pub(crate) fn validate_retry(
    previous: &Row,
    request: &RunChangeRequest,
    body: &serde_json::Value,
) -> Result<(), ControlError> {
    if previous.get("payload") != Some(body)
        || previous
            .get("invocation_id")
            .and_then(serde_json::Value::as_str)
            != Some(format!("scheduler/{}", request.run_id).as_str())
    {
        return Err(ControlError::Invalid(
            "run change event ID already has different content".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_authority(
    authority: RunAuthority<'_>,
    request: &RunChangeRequest,
) -> Result<(), ControlError> {
    let allowed = match authority {
        RunAuthority::Scheduler(_) => !matches!(
            request.change,
            RunChange::CancelRequested { .. }
                | RunChange::Started
                | RunChange::NodeStarted { .. }
                | RunChange::NodeFinished { .. }
        ),
        RunAuthority::Worker { run_id, claim_id } => {
            run_id == request.run_id
                && request.claim_id == Some(claim_id)
                && matches!(
                    request.change,
                    RunChange::Started
                        | RunChange::NodeStarted { .. }
                        | RunChange::NodeFinished { .. }
                        | RunChange::Recovered
                        | RunChange::Finished {
                            outcome: RunOutcome::Success
                                | RunOutcome::Warn
                                | RunOutcome::Partial
                                | RunOutcome::Failed
                                | RunOutcome::Cancelled,
                            ..
                        }
                )
        }
        RunAuthority::Operator(context) => {
            matches!(&request.change, RunChange::CancelRequested { actor, reason }
                if !context.actor.trim().is_empty() && context.actor == *actor
                    && !context.granting_role.trim().is_empty() && !reason.trim().is_empty())
        }
    };
    if !allowed {
        return Err(ControlError::Invalid(
            "authority cannot publish this run change".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_change(
    state: &ChangingRun,
    request: &RunChangeRequest,
) -> Result<(), ControlError> {
    use RunStatus::{Launched, Queued, Running, Unresponsive};
    if !matches!(state.status, Queued | Launched | Running | Unresponsive) {
        return Err(ControlError::Invalid("terminal runs cannot change".into()));
    }
    if !matches!(request.change, RunChange::CancelRequested { .. })
        && request.claim_id != state.worker_instance
    {
        return Err(ControlError::Invalid(
            "run change does not belong to the current claim".into(),
        ));
    }
    let valid = match request.change {
        RunChange::LaunchFailed { .. } => false,
        RunChange::Launched { .. } => state.worker_instance.is_some(),
        RunChange::Started => {
            state.worker_instance.is_some() && matches!(state.status, Queued | Launched)
        }
        RunChange::NodeStarted { .. } | RunChange::NodeFinished { .. } => {
            matches!(state.status, Running | Unresponsive)
        }
        RunChange::Unresponsive => state.status == Running,
        RunChange::Recovered => state.status == Unresponsive,
        RunChange::CancelRequested { .. } => state.cancel_requested_at.is_none(),
        RunChange::Finished { outcome, .. } => {
            state.worker_instance.is_some()
                || matches!(outcome, RunOutcome::Failed | RunOutcome::Cancelled)
        }
    };
    if !valid {
        return Err(ControlError::Invalid(
            "run change is inconsistent with the current lifecycle".into(),
        ));
    }
    if let RunChange::Finished {
        outcome: RunOutcome::Success | RunOutcome::Warn | RunOutcome::Partial,
        ..
    } = request.change
    {
        let count = |status: &str| {
            state
                .node_counts_json
                .get(status)
                .copied()
                .unwrap_or_default()
        };
        let good = count("success") + count("warn");
        let bad = count("failed") + count("skipped") + count("cancelled");
        let expected = if bad > 0 && good > 0 {
            RunOutcome::Partial
        } else if bad > 0 {
            RunOutcome::Failed
        } else if count("warn") > 0 {
            RunOutcome::Warn
        } else {
            RunOutcome::Success
        };
        if count("pending") > 0
            || count("running") > 0
            || !matches!(request.change, RunChange::Finished { outcome, .. } if outcome == expected)
        {
            return Err(ControlError::Invalid(
                "run outcome does not match its completed node facts".into(),
            ));
        }
    }
    Ok(())
}
