//! Named work and warehouse observations share the registered location/target boundary.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

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
