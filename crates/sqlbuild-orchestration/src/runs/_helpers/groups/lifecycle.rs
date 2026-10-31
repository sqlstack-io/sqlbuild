//! A virtual parent advances only from durable child evidence under the scheduler's fence.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use uuid::Uuid;

use crate::runs::_helpers::groups::admission::{earlier_waiter_blocks, group_capacity};
use crate::runs::_helpers::groups::context::load_group;
use crate::runs::_helpers::lifecycle::claims::reserve_claims;
use crate::runs::_helpers::lifecycle::publish_change::publish_change;
use crate::runs::models::{
    ChangingRun, GroupContext, NodeOutcome, QueueCandidate, RunChange, RunChangeRequest,
    RunNodeRecord, RunOutcome, RunStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

/// Callers have already checked leadership; the surrounding transaction fences the entire advance.
pub(crate) async fn group_change(
    transaction: ControlTransaction,
    run_id: Uuid,
    change: RunChange,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.status, r.cancel_requested_at, r.node_counts_json, l.worker_instance FROM sqb_runs r \
         LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id WHERE r.run_id=$1",
        vec![Parameter::Id(run_id)]).await?;
    let state: ChangingRun = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("group run is missing".into()))?,
    )?;
    let event_id = Uuid::new_v5(
        &run_id,
        &serde_json::to_vec(&change).map_err(|error| ControlError::Invalid(error.to_string()))?,
    );
    publish_change(
        transaction,
        &RunChangeRequest {
            event_id,
            run_id,
            claim_id: state.worker_instance,
            change,
        },
        &state,
    )
    .await
}

pub(crate) async fn advance_group(
    transaction: ControlTransaction,
    parent_id: Uuid,
    limits: &BTreeMap<String, NonZeroU32>,
    eligible: Option<&BTreeSet<(Uuid, String)>>,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, group) = load_group(transaction, parent_id).await?;
    let parent = &group.parent;
    let children = &group.children;
    if parent.state.status.is_terminal() {
        return Ok(transaction);
    }
    let stopped = group.stopped();
    if parent.state.cancel_requested_at.is_some() || stopped {
        for child in children {
            if !child.state.status.is_terminal() && child.state.cancel_requested_at.is_none() {
                transaction = group_change(
                    transaction,
                    child.request_json.run_id,
                    RunChange::CancelRequested {
                        actor: "system:group".into(),
                        reason: if stopped {
                            "group stopped after a child failure"
                        } else {
                            "parent group cancellation requested"
                        }
                        .into(),
                    },
                )
                .await?;
            }
        }
    } else if parent.state.status == RunStatus::Queued {
        let capacity;
        (transaction, capacity) = group_capacity(transaction, limits, eligible).await?;
        let blocked;
        (transaction, blocked) = earlier_waiter_blocks(
            transaction,
            &capacity,
            &QueueCandidate {
                request_json: parent.request_json.clone(),
                queued_at: parent.queued_at,
            },
        )
        .await?;
        if blocked || !capacity.safe(&parent.request_json) {
            return Ok(transaction);
        }
        (transaction, _) = query(transaction, "SAVEPOINT group_claim", vec![]).await?;
        let reserved;
        (transaction, reserved) = reserve_claims(transaction, &parent.request_json, limits).await?;
        if !reserved {
            (transaction, _) =
                query(transaction, "ROLLBACK TO SAVEPOINT group_claim", vec![]).await?;
        } else {
            transaction = group_change(transaction, parent_id, RunChange::Started).await?;
        }
        (transaction, _) = query(transaction, "RELEASE SAVEPOINT group_claim", vec![]).await?;
    }
    if !children.is_empty()
        && children
            .iter()
            .all(|child| child.state.status.is_terminal())
    {
        transaction = finish_group(transaction, &group, stopped).await?;
    }
    Ok(transaction)
}

async fn finish_group(
    transaction: ControlTransaction,
    group: &GroupContext,
    stopped: bool,
) -> Result<ControlTransaction, ControlError> {
    let parent = &group.parent;
    let parent_id = parent.request_json.run_id;
    let (mut transaction, rows) = query(transaction,
        "SELECT n.* FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id WHERE r.parent_run_id=$1 ORDER BY r.run_id, n.node_key",
        vec![Parameter::Id(parent_id)]).await?;
    let mut nodes: BTreeMap<String, RunNodeRecord> = BTreeMap::new();
    let rank = |status: &str| match status {
        "failed" => 5,
        "cancelled" => 4,
        "skipped" => 3,
        "warn" => 2,
        "success" => 1,
        _ => 0,
    };
    for row in rows {
        let node: RunNodeRecord = decode_row(row)?;
        if nodes
            .get(&node.node_key)
            .is_none_or(|previous| rank(&node.status) > rank(&previous.status))
        {
            nodes.insert(node.node_key.clone(), node);
        }
    }
    let mut outcomes: BTreeSet<String> = BTreeSet::new();
    for key in &parent.request_json.selection {
        let node = nodes
            .get(key)
            .ok_or_else(|| ControlError::Invalid("group node has no child evidence".into()))?;
        outcomes.insert(node.status.clone());
        let outcome: NodeOutcome = serde_json::from_value(serde_json::json!(node.status))
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        transaction = group_change(
            transaction,
            parent_id,
            RunChange::NodeFinished {
                node_key: key.clone(),
                outcome,
                rows_affected: node.rows_affected,
                error_code: node.error_code.clone(),
                skipped_because_of: node.skipped_because_of.clone(),
            },
        )
        .await?;
    }
    let good = outcomes.contains("success") || outcomes.contains("warn");
    let bad = group
        .children
        .iter()
        .any(|child| !matches!(child.state.status, RunStatus::Success | RunStatus::Warn))
        || outcomes
            .iter()
            .any(|status| !matches!(status.as_str(), "success" | "warn"));
    let outcome = if parent.state.cancel_requested_at.is_some() {
        RunOutcome::Cancelled
    } else if stopped {
        RunOutcome::Failed
    } else if bad && good {
        RunOutcome::Partial
    } else if bad {
        RunOutcome::Failed
    } else if outcomes.contains("warn")
        || group
            .children
            .iter()
            .any(|child| child.state.status == RunStatus::Warn)
    {
        RunOutcome::Warn
    } else {
        RunOutcome::Success
    };
    group_change(
        transaction,
        parent_id,
        RunChange::Finished {
            outcome,
            reason: "all group children have durable terminal outcomes".into(),
            exit_code: None,
        },
    )
    .await
}
