//! A run's event metadata needs view on every current node, as its node list does; payloads are never read.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::RunEventRecord;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{begin_run_recall, run_events_in};

/// Events after the storage cursor, oldest first; `None` when the run is not in this location and target.
pub async fn read_person_run_events(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (run, page): ((Uuid, &str, Uuid), (i64, u32)),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunEventRecord>>, ControlError> {
    let Some((transaction, scope, _)) =
        begin_run_recall(store, credentials, run, (default_role, None)).await?
    else {
        return Ok(None);
    };
    let (transaction, events) = run_events_in(transaction, &scope, page).await?;
    commit(transaction).await?;
    Ok(Some(events))
}
