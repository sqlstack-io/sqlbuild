//! A sensor run key gates queue publication within the cursor/outcome transaction.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::main::frozen_job_request::frozen_job_request;
use crate::runs::main::queue_run::queue_run;
use crate::runs::models::{JobRunRequest, RunOrigin};
use crate::sensors::models::SensorEvaluationRequest;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn queue_sensor_run(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    run_key: &str,
) -> Result<(ControlTransaction, Uuid), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT run_id FROM sqb_run_requests WHERE trigger_id=$1 AND run_key=$2",
        vec![
            Parameter::Id(evaluation.trigger_id),
            Parameter::Text(run_key.into()),
        ],
    )
    .await?;
    if let Some(row) = rows.into_iter().next() {
        return Ok((
            transaction,
            serde_json::from_value(row["run_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        ));
    }
    let run_id = Uuid::new_v5(&evaluation.trigger_id, run_key.as_bytes());
    let context = AuditContext {
        actor: "system:sensor".into(),
        granting_role: "system:scheduler".into(),
        request_id: evaluation.evaluation_id,
        ip_address: None,
    };
    let authored = JobRunRequest {
        run_id,
        location_id: evaluation.location_id,
        target: evaluation.target.clone(),
        job_key: evaluation.definition.job_key.clone(),
        reason: "Sensor condition matched".into(),
        priority: 0,
        window: None,
        full_refresh: false,
    };
    let (transaction, mut frozen) = frozen_job_request(transaction, &context, &authored).await?;
    if frozen.snapshot_id != evaluation.snapshot_id {
        return Err(ControlError::Fenced);
    }
    frozen.origin = RunOrigin::Sensor;
    frozen.trigger_id = Some(evaluation.trigger_id);
    let transaction = queue_run(transaction, &frozen).await?;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_requests (trigger_id,run_key,evaluation_id,run_id) VALUES ($1,$2,$3,$4)",
        vec![Parameter::Id(evaluation.trigger_id), Parameter::Text(run_key.into()), Parameter::Id(evaluation.evaluation_id), Parameter::Id(run_id)]).await?;
    Ok((transaction, run_id))
}
