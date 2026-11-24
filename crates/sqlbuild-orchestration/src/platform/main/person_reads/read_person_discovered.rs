//! A target's current discovered sets as a person reads them back, for example after a deploy.

use uuid::Uuid;

use crate::authorization::main::decisions::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use std::collections::BTreeMap;

use crate::discovered_sets::constants::{DISCOVERED_ASSETS_DOCUMENT, DISCOVERED_DOCUMENT};
use crate::discovered_sets::main::discovered_readback::discovered_readback;
use crate::discovered_sets::types::DiscoveredSets;
use crate::platform::models::DiscoveredReadback;
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::snapshots::main::reads::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

/// Each set's status, fetch time, digest and tags in the current snapshot; `None` before a publication.
pub async fn read_person_discovered(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Option<DiscoveredReadback>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        credentials,
        &PermissionScope::action(Action::View, Some((location_id, target))),
        default_role,
    )
    .await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let Some(snapshot) = snapshot else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (transaction, documents) = read_snapshot_documents(
        transaction,
        snapshot.snapshot_id,
        Some(&[DISCOVERED_DOCUMENT, DISCOVERED_ASSETS_DOCUMENT]),
    )
    .await?;
    commit(transaction).await?;
    let sets: DiscoveredSets = match documents.get(DISCOVERED_DOCUMENT) {
        Some(document) => serde_json::from_value(document.clone()).map_err(|error| {
            ControlError::Invalid(format!(
                "the snapshot's discovered sets are unreadable: {error}"
            ))
        })?,
        None => DiscoveredSets::new(),
    };
    let assets: BTreeMap<String, Vec<String>> = match documents.get(DISCOVERED_ASSETS_DOCUMENT) {
        Some(document) => serde_json::from_value(document.clone()).map_err(|error| {
            ControlError::Invalid(format!(
                "the snapshot's discovered set assets are unreadable: {error}"
            ))
        })?,
        None => BTreeMap::new(),
    };
    Ok(Some(DiscoveredReadback {
        snapshot_id: snapshot.snapshot_id,
        generation: snapshot.generation,
        published_at: snapshot.published_at,
        sets: discovered_readback(&sets, &assets),
    }))
}
