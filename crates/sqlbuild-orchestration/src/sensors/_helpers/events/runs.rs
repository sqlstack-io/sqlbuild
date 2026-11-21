//! Terminal projections carry the durable finish cursor; a retried member fires on its final attempt.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::runs::constants::SUPERSEDED_CHILD;
use crate::runs::models::RunOutcome;
use crate::sensors::_helpers::events::conditions::{page_clause, run_url};
use crate::sensors::constants::{
    CHECK_KEY_PREFIXES, CHECK_ROW_KIND, EVENT_PAGE_SIZE, EXECUTION_NODE_BATCH_SIZE,
};
use crate::sensors::models::{
    Direction, EventScope, RunEventKind, Scanned, SensorEvent, TerminalSensorRun,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn scan_run_events(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (direction, limit): (Direction, usize),
    (job_key, kinds, executed_successfully): (
        &str,
        &BTreeSet<RunEventKind>,
        Option<&BTreeSet<String>>,
    ),
) -> Result<(ControlTransaction, Scanned), ControlError> {
    let (clause, position) = page_clause(direction, "r.last_event_storage_id", 4);
    let (mut transaction, rows) = query(
        transaction,
        &format!("SELECT r.run_id,r.status,r.last_event_storage_id FROM sqb_runs r \
         WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
         AND r.status IN ('success','warn','partial','failed','cancelled','skipped') AND NOT {SUPERSEDED_CHILD} \
         AND {clause} LIMIT {EVENT_PAGE_SIZE}"),
        vec![
            Parameter::Id(scope.location_id),
            Parameter::Text(scope.target.into()),
            Parameter::Text(job_key.into()),
            position,
        ],
    )
    .await?;
    let mut scanned = Scanned {
        full: rows.len() >= EVENT_PAGE_SIZE,
        ..Scanned::default()
    };
    let runs: Vec<TerminalSensorRun> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let mut eligible: BTreeSet<Uuid> = runs.iter().map(|run| run.run_id).collect();
    if let Some(nodes) = executed_successfully {
        (transaction, eligible) = matching_executions(transaction, eligible, nodes).await?;
    }
    for run in runs {
        scanned.last = Some(run.last_event_storage_id);
        if eligible.contains(&run.run_id) && kinds.iter().any(|kind| kind.matches(run.status)) {
            let event;
            (transaction, event) = run_event(transaction, scope, (job_key, &run)).await?;
            scanned.events.push(event);
            if scanned.events.len() >= limit {
                break;
            }
        }
    }
    Ok((transaction, scanned))
}

/// The terminal event of one run of the sensor's job, or why the sensor's condition doesn't match it.
pub(crate) async fn run_event_for(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    run_id: Uuid,
    (job_key, kinds, executed_successfully): (
        &str,
        &BTreeSet<RunEventKind>,
        Option<&BTreeSet<String>>,
    ),
) -> Result<(ControlTransaction, Result<SensorEvent, String>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT r.run_id,r.status,r.last_event_storage_id,r.job_key FROM sqb_runs r \
         WHERE r.run_id=$1 AND r.location_id=$2 AND r.target=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(scope.location_id),
            Parameter::Text(scope.target.into()),
        ],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((
            transaction,
            Err(format!(
                "run {run_id} is not a run of target {}",
                scope.target
            )),
        ));
    };
    if row["job_key"].as_str() != Some(job_key) {
        return Ok((
            transaction,
            Err(format!(
                "run {run_id} is a run of {}, but the sensor watches {job_key}",
                row["job_key"].as_str().unwrap_or("an ad-hoc command")
            )),
        ));
    }
    let Ok(run) = decode_row::<TerminalSensorRun>(row.clone()) else {
        return Ok((
            transaction,
            Err(format!(
                "run {run_id} hasn't finished; it is {}",
                row["status"].as_str().unwrap_or("unknown")
            )),
        ));
    };
    if !kinds.iter().any(|kind| kind.matches(run.status)) {
        let watched: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
        return Ok((
            transaction,
            Err(format!(
                "run {run_id} ended {}, which the sensor's kinds ({}) don't match",
                run.status.as_str(),
                watched.join(", ")
            )),
        ));
    }
    let mut transaction = transaction;
    if let Some(nodes) = executed_successfully {
        let eligible;
        (transaction, eligible) =
            matching_executions(transaction, BTreeSet::from([run_id]), nodes).await?;
        if eligible.is_empty() {
            return Ok((
                transaction,
                Err(format!(
                    "run {run_id} didn't run and succeed every node the sensor's executed_successfully names"
                )),
            ));
        }
    }
    let (transaction, event) = run_event(transaction, scope, (job_key, &run)).await?;
    Ok((transaction, Ok(event)))
}

/// The run's outcome with its error and the nodes and checks that failed, its group members' included.
async fn run_event(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (job_key, run): (&str, &TerminalSensorRun),
) -> Result<(ControlTransaction, SensorEvent), ControlError> {
    let (transaction, reasons) = query(
        transaction,
        "SELECT status_reason, COALESCE(ended_at, queued_at) AS occurred_at FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(run.run_id)],
    )
    .await?;
    let reason = reasons.first();
    let error = reason
        .and_then(|row| row["status_reason"].as_str())
        .filter(|_| run.status != RunOutcome::Success)
        .map(str::to_owned);
    let occurred_at = match reason {
        Some(row) => serde_json::from_value(row["occurred_at"].clone())
            .map_err(|error| ControlError::Invalid(format!("run end time: {error}")))?,
        None => transaction.now,
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT DISTINCT n.node_key, n.kind FROM sqb_run_nodes n WHERE n.status='failed' \
         AND (n.run_id=$1 OR n.run_id IN (SELECT c.run_id FROM sqb_runs c WHERE c.parent_run_id=$1)) \
         ORDER BY n.node_key",
        vec![Parameter::Id(run.run_id)],
    )
    .await?;
    let failed = |checks: bool| -> Vec<String> {
        let mut keys: Vec<String> = rows
            .iter()
            .filter(|row| {
                let key = row["node_key"].as_str().unwrap_or_default();
                let check = row["kind"] == CHECK_ROW_KIND
                    || CHECK_KEY_PREFIXES
                        .iter()
                        .any(|prefix| key.starts_with(prefix));
                check == checks
            })
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect();
        keys.dedup();
        keys
    };
    Ok((
        transaction,
        SensorEvent::Run {
            id: format!("run:{}", run.run_id),
            occurred_at,
            job: job_key.into(),
            run_id: run.run_id,
            status: run.status,
            error,
            failed_nodes: failed(false),
            failed_checks: failed(true),
            target: scope.target.into(),
            url: run_url(scope.origin, run.run_id),
        },
    ))
}

/// Each chunk narrows the previous matches; a member's node qualifies from any attempt in its group run.
async fn matching_executions(
    mut transaction: ControlTransaction,
    mut eligible: BTreeSet<Uuid>,
    nodes: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeSet<Uuid>), ControlError> {
    let nodes: Vec<&String> = nodes.iter().collect();
    for chunk in nodes.chunks(EXECUTION_NODE_BATCH_SIZE) {
        if eligible.is_empty() {
            break;
        }
        let mut parameters: Vec<Parameter> = eligible.iter().copied().map(Parameter::Id).collect();
        let run_parameters = Parameter::placeholders(1..=parameters.len());
        let node_parameters = (parameters.len() + 1..=parameters.len() + chunk.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
        parameters.extend(chunk.iter().map(|key| Parameter::Text((*key).to_string())));
        let rows;
        (transaction, rows) = query(transaction, &format!(
            "SELECT r.run_id,r.status,r.last_event_storage_id FROM sqb_runs r \
             WHERE r.run_id IN ({run_parameters}) AND (\
             (SELECT COUNT(*) FROM sqb_run_nodes n WHERE n.run_id=r.run_id AND n.kind='node' \
              AND n.node_key IN ({node_parameters}) AND n.status='success' AND n.ended_at IS NOT NULL \
              AND ((r.kind<>'group' AND n.started_at IS NOT NULL) OR \
                   (r.kind='group' AND EXISTS(SELECT 1 FROM sqb_runs c \
                    JOIN sqb_run_nodes cn ON cn.run_id=c.run_id AND cn.node_key=n.node_key AND cn.kind='node' \
                    WHERE c.parent_run_id=r.run_id AND c.location_id=r.location_id AND c.target=r.target \
                    AND cn.status='success' AND cn.started_at IS NOT NULL AND cn.ended_at IS NOT NULL))))={count} \
             OR (r.parent_run_id IS NOT NULL AND \
              (SELECT COUNT(DISTINCT an.node_key) FROM sqb_runs a JOIN sqb_run_nodes an ON an.run_id=a.run_id AND an.kind='node' \
               WHERE a.parent_run_id=r.parent_run_id AND a.job_key=r.job_key AND an.node_key IN ({node_parameters}) \
               AND an.status='success' AND an.started_at IS NOT NULL AND an.ended_at IS NOT NULL)={count}))",
            count = chunk.len()), parameters).await?;
        let matches: Vec<TerminalSensorRun> =
            rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
        eligible = matches.into_iter().map(|run| run.run_id).collect();
    }
    Ok((transaction, eligible))
}
