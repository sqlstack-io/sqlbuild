//! A stored republish request and its location's result, as the API reports it.

use chrono::{DateTime, Utc};

use crate::platform::models::{
    LocationWork, LocationWorkOutcome, LocationWorkRecord, LocationWorkRequest, RepublishState,
    RepublishStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// `record` as a republish's status at `now`; `None` when it is other work.
pub(crate) fn republish_status(
    record: &LocationWorkRecord,
    now: DateTime<Utc>,
) -> Option<RepublishStatus> {
    let LocationWork::Republish { requested_by } = &record.request_json.work else {
        return None;
    };
    let mut status = RepublishStatus {
        request_id: record.request_json.request_id,
        location_id: record.request_json.location_id,
        target: record.request_json.target.clone(),
        state: RepublishState::Pending,
        requested_by: requested_by.clone(),
        reason: record.request_json.reason.clone(),
        requested_at: record.requested_at,
        expires_at: record.expires_at,
        completed_at: record.completed_at,
        snapshot_id: None,
        generation: None,
        error: None,
    };
    match record.result_json.as_ref().map(|result| &result.outcome) {
        Some(LocationWorkOutcome::Published {
            snapshot_id,
            generation,
        }) => {
            status.state = RepublishState::Published;
            status.snapshot_id = Some(*snapshot_id);
            status.generation = Some(*generation);
        }
        Some(LocationWorkOutcome::PublishFailed { generation, error }) => {
            status.state = RepublishState::Failed;
            status.generation = Some(*generation);
            status.error = Some(error.clone());
        }
        Some(_) => {
            status.state = RepublishState::Failed;
            status.error = Some("the location could not republish".to_owned());
        }
        None if record.expires_at <= now => status.state = RepublishState::Expired,
        None => {}
    }
    Some(status)
}

/// Whether a republish's snapshot is one this location published for the work's target; true for other outcomes.
pub(crate) async fn published_here(
    transaction: ControlTransaction,
    request: &LocationWorkRequest,
    outcome: &LocationWorkOutcome,
) -> Result<(ControlTransaction, bool), ControlError> {
    let LocationWorkOutcome::Published {
        snapshot_id,
        generation,
    } = outcome
    else {
        return Ok((transaction, true));
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_snapshots WHERE snapshot_id=$1 AND location_id=$2 AND target=$3 AND generation=$4",
        vec![
            Parameter::Id(*snapshot_id),
            Parameter::Id(request.location_id),
            Parameter::Text(request.target.clone()),
            Parameter::Integer(*generation),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
