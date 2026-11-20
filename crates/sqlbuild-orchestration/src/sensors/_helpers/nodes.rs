//! Node and check conditions read executed run-node outcomes in finish order.

use std::collections::BTreeSet;

use crate::runs::models::NodeOutcome;
use crate::sensors::_helpers::conditions::{page_clause, run_url};
use crate::sensors::_helpers::transitions::matching_transition;
use crate::sensors::constants::EVENT_PAGE_SIZE;
use crate::sensors::models::{
    CheckEventKind, Direction, EventScope, NodeEventKind, NodeSensorEvent, Scanned, SensorEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// One page of finished rows of `kind` (`node` or `check`) in the scope, in `direction`'s order.
async fn finished_rows(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    direction: Direction,
    (kind, statuses): (&str, &str),
) -> Result<(ControlTransaction, Vec<NodeSensorEvent>, bool), ControlError> {
    let (clause, position) = page_clause(direction, "n.finished_storage_id", 1);
    let started = if kind == "node" {
        " AND n.started_at IS NOT NULL AND r.kind<>'group'"
    } else {
        ""
    };
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT n.run_id,n.node_key,n.status,n.finished_storage_id,r.job_key,\
             COALESCE(n.ended_at, r.ended_at, r.queued_at) AS occurred_at FROM sqb_run_nodes n \
             JOIN sqb_runs r ON r.run_id=n.run_id \
             WHERE n.finished_storage_id IS NOT NULL AND n.kind='{kind}' AND n.status IN ({statuses}){started} \
             AND r.location_id=$2 AND r.target=$3 AND {clause} LIMIT {EVENT_PAGE_SIZE}"
        ),
        vec![
            position,
            Parameter::Id(scope.location_id),
            Parameter::Text(scope.target.into()),
        ],
    )
    .await?;
    let full = rows.len() >= EVENT_PAGE_SIZE;
    let rows: Vec<NodeSensorEvent> = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    Ok((transaction, rows, full))
}

pub(crate) async fn scan_node_events(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (direction, limit): (Direction, usize),
    (selection, kinds): (&BTreeSet<String>, &BTreeSet<NodeEventKind>),
) -> Result<(ControlTransaction, Scanned), ControlError> {
    let (mut transaction, rows, full) = finished_rows(
        transaction,
        scope,
        direction,
        ("node", "'success','failed'"),
    )
    .await?;
    let mut scanned = Scanned {
        full,
        ..Scanned::default()
    };
    for node in rows {
        scanned.last = Some(node.finished_storage_id);
        if !selection.contains(&node.node_key) {
            continue;
        }
        let matched;
        (transaction, matched) = matching_transition(transaction, scope, &node, kinds).await?;
        let Some(event) = matched else { continue };
        scanned.events.push(SensorEvent::Node {
            id: format!("node:{}:{}", node.run_id, node.node_key),
            occurred_at: node.occurred_at,
            run_id: node.run_id,
            job: node.job_key,
            node: node.node_key,
            event,
            target: scope.target.into(),
            url: run_url(scope.origin, node.run_id),
        });
        if scanned.events.len() >= limit {
            break;
        }
    }
    Ok((transaction, scanned))
}

/// A check, audit or SQL test that failed or warned inside a run, whatever the run's outcome.
pub(crate) async fn scan_check_events(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (direction, limit): (Direction, usize),
    (selection, kinds): (&BTreeSet<String>, &BTreeSet<CheckEventKind>),
) -> Result<(ControlTransaction, Scanned), ControlError> {
    let (transaction, rows, full) =
        finished_rows(transaction, scope, direction, ("check", "'failed','warn'")).await?;
    let mut scanned = Scanned {
        full,
        ..Scanned::default()
    };
    for check in rows {
        scanned.last = Some(check.finished_storage_id);
        let status = match check.status {
            NodeOutcome::Failed => CheckEventKind::Failed,
            NodeOutcome::Warn => CheckEventKind::Warn,
            _ => continue,
        };
        if !selection.contains(&check.node_key) || !kinds.contains(&status) {
            continue;
        }
        scanned.events.push(SensorEvent::Check {
            id: format!("check:{}:{}", check.run_id, check.node_key),
            occurred_at: check.occurred_at,
            run_id: check.run_id,
            job: check.job_key,
            check: check.node_key,
            status,
            target: scope.target.into(),
            url: run_url(scope.origin, check.run_id),
        });
        if scanned.events.len() >= limit {
            break;
        }
    }
    Ok((transaction, scanned))
}
