//! Read durable work inside a caller's existing transaction, including fenced sensor completion.

use uuid::Uuid;

use crate::platform::models::StoredLocationWork;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_location_work(
    transaction: ControlTransaction,
    request_id: Uuid,
) -> Result<(ControlTransaction, Option<StoredLocationWork>), ControlError> {
    let (transaction, records) = query_records(
        transaction,
        "SELECT w.request_json, w.requested_by, w.requested_at, w.expires_at, w.generation, \
         w.claim_id, w.session_id, w.claimed_until, w.result_json, w.completed_at \
         FROM sqb_location_work w \
         WHERE w.request_id=$1",
        vec![Parameter::Id(request_id)],
    )
    .await?;
    Ok((transaction, records.into_iter().next()))
}
