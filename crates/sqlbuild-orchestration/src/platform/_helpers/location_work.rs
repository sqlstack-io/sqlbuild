//! Request retries and completion share the same immutable result reader and instance scope.

use uuid::Uuid;

use crate::platform::models::StoredLocationWork;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_work(
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

pub(crate) async fn require_instance(
    transaction: ControlTransaction,
    (location_id, session_id, target): (Uuid, Uuid, &str),
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT i.instance_id FROM sqb_location_instances i JOIN sqb_locations l ON l.location_id=i.location_id \
         WHERE i.instance_id=$1 AND i.location_id=$2 AND i.target=$3 AND l.archived_at IS NULL",
        vec![Parameter::Id(session_id), Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    if rows.is_empty() {
        return Err(ControlError::Invalid(
            "a registered instance of this location and target is required".into(),
        ));
    }
    Ok(transaction)
}
