//! Read one exact latest build through the node/destination index, never infer live state.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::fingerprint_reads::latest_fingerprint;
use crate::warehouse_metadata::models::{FingerprintObservation, FingerprintQuery};

pub async fn read_person_fingerprint(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &FingerprintQuery),
    default_role: DefaultRole,
) -> Result<Option<FingerprintObservation>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        credentials,
        &PermissionScope {
            action: Action::View,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes: [format!("{}:{}", options.node_type, options.node_name)].into(),
        },
        default_role,
    )
    .await?;
    let (transaction, evidence) =
        latest_fingerprint(transaction, (location_id, target, options)).await?;
    commit(transaction).await?;
    Ok(evidence.map(|(record, _, _)| record))
}
