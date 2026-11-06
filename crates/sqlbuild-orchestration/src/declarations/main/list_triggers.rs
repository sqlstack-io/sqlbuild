//! Current declarations and runtime state are read in one consistent transaction.

use uuid::Uuid;

use crate::declarations::models::TriggerRecord;
use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

pub async fn list_triggers(
    store: &ControlStore,
    location_id: Uuid,
    target: &str,
) -> Result<Vec<TriggerRecord>, ControlError> {
    read_records(store,
        "SELECT t.trigger_id, t.name, t.job_key, t.definition_json, s.status, s.last_tick_for, s.next_due_at \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         WHERE t.location_id=$1 AND t.target=$2 AND t.removed_in_snapshot_id IS NULL ORDER BY t.name",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await
}
