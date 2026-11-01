//! Terminal projections carry the durable finish cursor; late observations cannot move it.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::sensors::_helpers::requests::queue_sensor_run;
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
    job_key: &str,
    kinds: &BTreeSet<RunEventKind>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT run_id,status,last_event_storage_id FROM sqb_runs \
         WHERE location_id=$1 AND target=$2 AND job_key=$3 AND last_event_storage_id>$4 \
         AND status IN ('success','warn','partial','failed','cancelled','presumed_failed') \
         ORDER BY last_event_storage_id LIMIT 100",
        vec![
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
            Parameter::Text(job_key.into()),
            Parameter::Integer(evaluation.cursor.event_storage_id),
        ],
    )
    .await?;
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for row in rows {
        let run: TerminalSensorRun = decode_row(row)?;
        cursor.event_storage_id = run.last_event_storage_id;
        if kinds.iter().any(|kind| kind.matches(run.status)) {
            let run_id;
            (transaction, run_id) =
                queue_sensor_run(transaction, evaluation, &run.run_id.to_string()).await?;
            run_ids.push(run_id);
        }
    }
    Ok((transaction, (cursor, run_ids)))
}
