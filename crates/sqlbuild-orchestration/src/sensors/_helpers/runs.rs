//! Terminal projections carry the durable finish cursor; late observations cannot move it.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::constants::EXECUTION_NODE_BATCH_SIZE;
use crate::sensors::models::{
    RunEventKind, SensorCursor, SensorEvaluationRequest, TerminalSensorRun,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn consume_run_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    (job_key, kinds): (&str, &BTreeSet<RunEventKind>),
    executed_successfully: Option<&BTreeSet<String>>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT run_id,status,last_event_storage_id FROM sqb_runs \
         WHERE location_id=$1 AND target=$2 AND job_key=$3 AND last_event_storage_id>$4 \
         AND status IN ('success','warn','partial','failed','cancelled') \
         ORDER BY last_event_storage_id LIMIT 100",
        vec![
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
            Parameter::Text(job_key.into()),
            Parameter::Integer(evaluation.cursor.event_storage_id),
        ],
    )
    .await?;
    let runs: Vec<TerminalSensorRun> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let mut eligible: BTreeSet<Uuid> = runs.iter().map(|run| run.run_id).collect();
    if let Some(nodes) = executed_successfully {
        (transaction, eligible) = matching_executions(transaction, eligible, nodes).await?;
    }
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for run in runs {
        cursor.event_storage_id = run.last_event_storage_id;
        if eligible.contains(&run.run_id) && kinds.iter().any(|kind| kind.matches(run.status)) {
            let run_id;
            (transaction, run_id) =
                queue_sensor_run(transaction, evaluation, &run.run_id.to_string()).await?;
            run_ids.push(run_id);
        }
    }
    Ok((transaction, (cursor, run_ids)))
}

/// Each chunk intersects the previous matches: every selected node must qualify.
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
        let run_parameters = (1..=parameters.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
        let node_parameters = (parameters.len() + 1..=parameters.len() + chunk.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
        parameters.extend(chunk.iter().map(|key| Parameter::Text((*key).to_string())));
        let rows;
        (transaction, rows) = query(transaction, &format!(
            "SELECT r.run_id,r.status,r.last_event_storage_id FROM sqb_runs r \
             WHERE r.run_id IN ({run_parameters}) AND \
             (SELECT COUNT(*) FROM sqb_run_nodes n WHERE n.run_id=r.run_id AND n.kind='node' \
              AND n.node_key IN ({node_parameters}) AND n.status='success' AND n.ended_at IS NOT NULL \
              AND ((r.kind<>'group' AND n.started_at IS NOT NULL) OR \
                   (r.kind='group' AND EXISTS(SELECT 1 FROM sqb_runs c \
                    JOIN sqb_run_nodes cn ON cn.run_id=c.run_id AND cn.node_key=n.node_key AND cn.kind='node' \
                    WHERE c.parent_run_id=r.run_id AND c.location_id=r.location_id AND c.target=r.target \
                    AND cn.status='success' AND cn.started_at IS NOT NULL AND cn.ended_at IS NOT NULL))))={}",
            chunk.len()), parameters).await?;
        let matches: Vec<TerminalSensorRun> =
            rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
        eligible = matches.into_iter().map(|run| run.run_id).collect();
    }
    Ok((transaction, eligible))
}
