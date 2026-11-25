//! A person lists the newest result of every check with view on the whole target, served from the held list while no signal has named the target.

use std::ops::ControlFlow;

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::node_states::_helpers::bodies::checks_json;
use crate::node_states::_helpers::checks::{held_checks, keep_checks};
use crate::node_states::_helpers::views::target_cache;
use crate::node_states::models::{CachedJson, HeldCheckList, NodeStateCache};
use crate::snapshots::main::reads::listing_read::listing_read;
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;
use crate::warehouse_history::main::checks::read_latest_checks::read_latest_checks;

/// Authorizes every read; a held list of the current snapshot is served as it is, otherwise the results are read and the list kept unless a signal arrived meanwhile.
pub async fn list_person_checks(
    cache: &NodeStateCache,
    credentials: (&AuditContext, Uuid),
    scope: (Uuid, &str),
    default_role: DefaultRole,
) -> Result<CachedJson, ControlError> {
    let entry = target_cache(cache, scope);
    let (generation, held) = held_checks(&entry);
    let (snapshot_id, read) = listing_read(
        &cache.store,
        (credentials, default_role),
        scope,
        |transaction| async {
            let (transaction, snapshot) = read_current_record(transaction, scope).await?;
            let snapshot_id = snapshot.map(|snapshot| snapshot.snapshot_id);
            if let Some(held) = held.filter(|held| held.snapshot_id == snapshot_id) {
                return Ok((transaction, (snapshot_id, ControlFlow::Break(held.json))));
            }
            let (transaction, results) = read_latest_checks(transaction, scope).await?;
            Ok((transaction, (snapshot_id, ControlFlow::Continue(results))))
        },
    )
    .await?;
    let results = match read {
        ControlFlow::Break(held) => return Ok(held),
        ControlFlow::Continue(results) => results,
    };
    let json = checks_json(snapshot_id, results).await?;
    keep_checks(
        &entry,
        generation,
        HeldCheckList {
            snapshot_id,
            json: json.clone(),
        },
    );
    Ok(json)
}
