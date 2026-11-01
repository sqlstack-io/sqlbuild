//! Sensor publication keeps operator overrides and immutable cursor history across deployments.

use serde_json::json;
use uuid::Uuid;

use crate::declarations::models::Definitions;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn publish_sensors(
    mut transaction: ControlTransaction,
    snapshot: &SnapshotRecord,
    definitions: &Definitions,
) -> Result<ControlTransaction, ControlError> {
    for (name, sensor) in &definitions.sensors {
        let trigger_id = Uuid::new_v5(
            &snapshot.location_id,
            &serde_json::to_vec(&(&snapshot.target, "sensor", name))
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_triggers (trigger_id,location_id,target,kind,name,job_key,definition_json,snapshot_id) \
             VALUES ($1,$2,$3,'sensor',$4,$5,$6,$7) ON CONFLICT (trigger_id) DO UPDATE \
             SET job_key=excluded.job_key,definition_json=excluded.definition_json,snapshot_id=excluded.snapshot_id,removed_in_snapshot_id=NULL",
            vec![Parameter::Id(trigger_id), Parameter::Id(snapshot.location_id), Parameter::Text(snapshot.target.clone()),
                Parameter::Text(name.clone()), Parameter::Text(sensor.job_key.clone()), Parameter::Json(json!(sensor)), Parameter::Id(snapshot.snapshot_id)]).await?;
        let status = sensor.default_status.projection();
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_trigger_state (trigger_id,status,started_at,next_due_at,updated_at) VALUES ($1,$2,$3,$3,$3) \
             ON CONFLICT (trigger_id) DO UPDATE SET status=COALESCE(sqb_trigger_state.status_override,excluded.status), \
             next_due_at=excluded.next_due_at,updated_at=excluded.updated_at",
            vec![Parameter::Id(trigger_id), Parameter::Text(status.as_str().into()), Parameter::Time(snapshot.published_at)]).await?;
    }
    Ok(transaction)
}
