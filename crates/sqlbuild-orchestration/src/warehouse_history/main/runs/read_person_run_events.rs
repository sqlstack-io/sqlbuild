//! A run's event metadata needs view on every current node, as its node list does; payloads are never read.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::RunEventRecord;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{run_events_in, run_read};

/// Events after the storage cursor, oldest first; `None` when the run is not in this location and target.
pub async fn read_person_run_events(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (run, page): ((Uuid, &str, Uuid), (i64, u32)),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunEventRecord>>, ControlError> {
    run_read(
        store,
        credentials,
        (run, default_role),
        async |transaction, scope| run_events_in(transaction, scope, page).await,
    )
    .await
}
