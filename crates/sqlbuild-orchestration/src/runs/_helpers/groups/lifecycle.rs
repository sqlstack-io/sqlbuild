//! A virtual parent advances only from durable child evidence under the scheduler's fence.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde_json::Value;
use uuid::Uuid;

use crate::declarations::models::DependencyFailure;
use crate::runs::_helpers::groups::admission::{
    admission_limits, admitted, earlier_waiter_blocks, limit_waiters,
};
use crate::runs::_helpers::groups::context::load_group;
use crate::runs::_helpers::groups::selection::{failed_upstream, member_dependencies};
use crate::runs::_helpers::lifecycle::claims::reserve_claims;
use crate::runs::_helpers::lifecycle::publish_change::publish_change;
use crate::runs::models::{
    ChangingRun, GroupContext, NodeOutcome, QueueCandidate, RunChange, RunChangeRequest,
    RunNodeRecord, RunOutcome, RunStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// Callers have already checked leadership; the surrounding transaction fences the entire advance.
pub(crate) async fn group_change(
    transaction: ControlTransaction,
    run_id: Uuid,
    change: RunChange,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.status, r.cancel_requested_at, r.node_counts_json, r.worker_instance FROM sqb_runs r WHERE r.run_id=$1",
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
    } else if parent.state.status == RunStatus::Running {
        transaction = skip_blocked_members(transaction, &group).await?;
    } else if parent.state.status == RunStatus::Queued && parent.request_json.due(transaction.now) {
        let admission = admitted(
            &parent.request_json,
            children.iter().map(|child| &child.request_json),
        );
        let wanted = admission_limits(
            &parent.request_json,
            children.iter().map(|child| &child.request_json),
        );
        let waiters;
        (transaction, waiters) = limit_waiters(transaction, eligible).await?;
        let candidate = QueueCandidate {
            request_json: parent.request_json.clone(),
            queued_at: parent.queued_at,
        };
        if earlier_waiter_blocks(&waiters, &candidate, &wanted) {
            return Ok(transaction);
        }
        (transaction, _) = query(transaction, "SAVEPOINT group_claim", vec![]).await?;
        let reserved;
        (transaction, reserved) = reserve_claims(transaction, &admission, limits).await?;
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

/// End each queued member whose policy is `skip` once a job it depends on ended without success.
async fn skip_blocked_members(
    mut transaction: ControlTransaction,
    group: &GroupContext,
) -> Result<ControlTransaction, ControlError> {
    let Some(declaration) = group
        .parent
        .request_json
        .job_key
        .as_ref()
        .and_then(|name| group.definitions.groups.get(name))
    else {
        return Ok(transaction);
    };
    for child in &group.children {
        let request = &child.request_json;
        let Some(name) = request.job_key.as_deref() else {
            continue;
        };
        if child.state.status != RunStatus::Queued
            || child.state.cancel_requested_at.is_some()
            || declaration.member_dependency_failure(name) != DependencyFailure::Skip
        {
            continue;
        }
        let Some(failed) = failed_upstream(group, &member_dependencies(group, name)) else {
            continue;
        };
        let failed_job = failed.request_json.job_key.clone().unwrap_or_default();
        let rows;
        (transaction, rows) = query(transaction,
            "SELECT n.node_key, n.skipped_because_of FROM sqb_run_nodes n WHERE n.run_id=$1 AND n.kind='node' \
             AND n.status IN ('failed','skipped','cancelled') ORDER BY CASE n.status WHEN 'failed' THEN 0 ELSE 1 END, n.node_key LIMIT 1",
            vec![Parameter::Id(failed.request_json.run_id)]).await?;
        let cause: Option<String> = rows.first().and_then(|row| {
            row["skipped_because_of"]
                .as_str()
                .or_else(|| row["node_key"].as_str())
                .map(str::to_owned)
        });
        for node_key in &request.selection {
            transaction = group_change(
                transaction,
                request.run_id,
                RunChange::NodeFinished {
                    node_key: node_key.clone(),
                    outcome: NodeOutcome::Skipped,
                    rows_affected: None,
                    error_code: None,
                    skipped_because_of: cause.clone(),
                    error_message: None,
                },
            )
            .await?;
        }
        transaction = group_change(
            transaction,
            request.run_id,
            RunChange::Finished {
                outcome: RunOutcome::Skipped,
                reason: format!("skipped because {failed_job} did not succeed"),
                exit_code: None,
            },
        )
        .await?;
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
        "SELECT n.*, r.rerun_of_run_id AS previous_attempt FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE r.parent_run_id=$1 AND n.kind='node' ORDER BY r.run_id, n.node_key",
        vec![Parameter::Id(parent_id)]).await?;
    let mut latest: BTreeMap<(Uuid, String), (usize, RunNodeRecord)> = BTreeMap::new();
    let previous: BTreeMap<Uuid, Uuid> = rows
        .iter()
        .map(previous_attempt)
        .filter_map(Result::transpose)
        .collect::<Result<_, ControlError>>()?;
    let runs: BTreeSet<Uuid> = rows
        .iter()
        .map(|row| stored_id(&row["run_id"]))
        .collect::<Result<_, ControlError>>()?;
    for row in rows {
        let run: Uuid = serde_json::from_value(row["run_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let (mut first, mut attempt) = (run, 1);
        while let Some(earlier) = previous
            .get(&first)
            .filter(|earlier| runs.contains(*earlier))
        {
            (first, attempt) = (*earlier, attempt + 1);
        }
        let node: RunNodeRecord = decode_row(row)?;
        let key = (first, node.node_key.clone());
        if latest.get(&key).is_none_or(|(seen, _)| attempt > *seen) {
            latest.insert(key, (attempt, node));
        }
    }
    let mut nodes: BTreeMap<String, RunNodeRecord> = BTreeMap::new();
    let rank = |status: &str| match status {
        "failed" => 5,
        "cancelled" => 4,
        "skipped" => 3,
        "warn" => 2,
        "success" => 1,
        _ => 0,
    };
    for (_, node) in latest.into_values() {
        if nodes
            .get(&node.node_key)
            .is_none_or(|previous| rank(&node.status) > rank(&previous.status))
        {
            nodes.insert(node.node_key.clone(), node);
        }
    }
    let mut outcomes: BTreeSet<String> = BTreeSet::new();
    let mut good = false;
    let build = parent.request_json.command.is_build();
    let source_needs_no_run = |key: &str| build && key.starts_with("source:");
    for key in &parent.request_json.selection {
        let node = nodes
            .get(key)
            .ok_or_else(|| ControlError::Invalid("group node has no child evidence".into()))?;
        outcomes.insert(node.status.clone());
        good |= matches!(node.status.as_str(), "success" | "warn") && !source_needs_no_run(key);
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
                error_message: None,
            },
        )
        .await?;
    }
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
    let ignored: i64 = group
        .children
        .iter()
        .map(|child| child.state.ignored_audit_failures)
        .sum();
    let mut transaction = transaction;
    if ignored > 0 {
        transaction = group_change(
            transaction,
            parent_id,
            RunChange::AuditFailuresIgnored {
                count: u32::try_from(ignored).unwrap_or(u32::MAX),
            },
        )
        .await?;
    }
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

/// A stored run ID column.
fn stored_id(value: &Value) -> Result<Uuid, ControlError> {
    serde_json::from_value(value.clone()).map_err(|error| ControlError::Invalid(error.to_string()))
}

/// A member run and the earlier attempt it reruns, when it reruns one.
fn previous_attempt(row: &Row) -> Result<Option<(Uuid, Uuid)>, ControlError> {
    if row["previous_attempt"].is_null() {
        return Ok(None);
    }
    Ok(Some((
        stored_id(&row["run_id"])?,
        stored_id(&row["previous_attempt"])?,
    )))
}
