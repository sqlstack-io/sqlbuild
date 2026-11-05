//! One bounded platform page yields matched run requests and a storage-order cursor.

use uuid::Uuid;

use crate::platform::main::read_platform_events::read_platform_events;
use crate::platform::models::PlatformEventKind;
use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::models::{SensorCursor, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn consume_platform_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    kinds: &std::collections::BTreeSet<PlatformEventKind>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, events) =
        read_platform_events(transaction, evaluation.cursor.event_storage_id).await?;
    let cursor = SensorCursor {
        event_storage_id: events
            .last()
            .map_or(evaluation.cursor.event_storage_id, |event| event.storage_id),
        ..evaluation.cursor.clone()
    };
    let mut run_ids: Vec<Uuid> = Vec::new();
    for event in events
        .iter()
        .filter(|event| kinds.contains(&event.payload.platform.kind()))
    {
        let run_id;
        (transaction, run_id) =
            queue_sensor_run(transaction, evaluation, &event.event_id.to_string()).await?;
        run_ids.push(run_id);
    }
    Ok((transaction, (cursor, run_ids)))
}
