//! Read durable work inside a caller's existing transaction, including fenced sensor completion.

use uuid::Uuid;

use crate::platform::models::StoredLocationWork;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_location_work(
    transaction: ControlTransaction,
    request_id: Uuid,
) -> Result<(ControlTransaction, Option<StoredLocationWork>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT w.request_json, w.requested_by, w.requested_at, w.expires_at, w.generation, \
         w.claim_id, w.session_id, w.claimed_until, r.result_json, r.completed_at \
         FROM sqb_location_work w LEFT JOIN sqb_location_work_results r ON r.request_id=w.request_id \
         WHERE w.request_id=$1", vec![Parameter::Id(request_id)]).await?;
    Ok((
        transaction,
        rows.into_iter().next().map(decode_row).transpose()?,
    ))
}
