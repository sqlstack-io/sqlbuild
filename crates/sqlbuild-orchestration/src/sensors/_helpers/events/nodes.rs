//! Node and check conditions read executed run-node outcomes in finish order.

use std::collections::BTreeSet;

use crate::runs::models::NodeOutcome;
use crate::sensors::_helpers::events::conditions::{page_clause, run_url};
use crate::sensors::_helpers::ticks::transitions::matching_transition;
use crate::sensors::constants::{CHECK_EVENT_ROWS, EVENT_PAGE_SIZE, NODE_EVENT_ROWS};
use crate::sensors::models::{
    CheckEventKind, Direction, EventScope, NodeEventKind, NodeSensorEvent, Scanned, SensorEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// One page of finished rows in the scope that `rows` admits, in `direction`'s order.
async fn finished_rows(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    direction: Direction,
    (rows, statuses): (&str, &str),
) -> Result<(ControlTransaction, Vec<NodeSensorEvent>, bool), ControlError> {
    let (clause, position) = page_clause(direction, "n.finished_storage_id", 1);
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT n.run_id,n.node_key,n.status,n.finished_storage_id,r.job_key,\
             COALESCE(n.ended_at, r.ended_at, r.queued_at) AS occurred_at FROM sqb_run_nodes n \
             JOIN sqb_runs r ON r.run_id=n.run_id \
             WHERE n.finished_storage_id IS NOT NULL AND n.status IN ({statuses}){rows} \
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
        (NODE_EVENT_ROWS, "'success','failed'"),
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
        scanned
            .events
            .extend(matched.map(|event| node_event(scope, node, event)));
        if scanned.events.len() >= limit {
            break;
        }
    }
    Ok((transaction, scanned))
}

fn node_event(scope: EventScope<'_>, node: NodeSensorEvent, event: NodeEventKind) -> SensorEvent {
    SensorEvent::Node {
        id: format!("node:{}:{}", node.run_id, node.node_key),
        occurred_at: node.occurred_at,
        run_id: node.run_id,
        job: node.job_key,
        node: node.node_key,
        event,
        target: scope.target.into(),
        url: run_url(scope.origin, node.run_id),
    }
}

/// A check, audit or SQL test that failed or warned inside a run, whatever the run's outcome.
pub(crate) async fn scan_check_events(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (direction, limit): (Direction, usize),
    (selection, kinds): (&BTreeSet<String>, &BTreeSet<CheckEventKind>),
) -> Result<(ControlTransaction, Scanned), ControlError> {
    let (transaction, rows, full) = finished_rows(
        transaction,
        scope,
        direction,
        (CHECK_EVENT_ROWS, "'failed','warn'"),
    )
    .await?;
    let mut scanned = Scanned {
        full,
        ..Scanned::default()
    };
    for check in rows {
        scanned.last = Some(check.finished_storage_id);
        scanned
            .events
            .extend(check_event(scope, check, (selection, kinds)));
        if scanned.events.len() >= limit {
            break;
        }
    }
    Ok((transaction, scanned))
}

/// The check's outcome as an event, when it is one of `kinds` on a selected check.
fn check_event(
    scope: EventScope<'_>,
    check: NodeSensorEvent,
    (selection, kinds): (&BTreeSet<String>, &BTreeSet<CheckEventKind>),
) -> Option<SensorEvent> {
    let status = match check.status {
        NodeOutcome::Failed => CheckEventKind::Failed,
        NodeOutcome::Warn => CheckEventKind::Warn,
        _ => return None,
    };
    (selection.contains(&check.node_key) && kinds.contains(&status)).then(|| SensorEvent::Check {
        id: format!("check:{}:{}", check.run_id, check.node_key),
        occurred_at: check.occurred_at,
        run_id: check.run_id,
        job: check.job_key,
        check: check.node_key,
        status,
        target: scope.target.into(),
        url: run_url(scope.origin, check.run_id),
    })
}
