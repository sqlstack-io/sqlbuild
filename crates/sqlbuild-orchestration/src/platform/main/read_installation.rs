//! Administrators read when the control store was created and which schema it holds.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::platform::models::{InstallationRecord, StorageKind, StoredInstallation};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query_records::query_records};
use crate::store::models::{ControlStore, ControlTransaction};

/// Requires `user.manage` everywhere, as the people list does.
pub async fn read_installation(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
) -> Result<InstallationRecord, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        credentials,
        &PermissionScope::action(Action::UserManage, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, rows): (ControlTransaction, Vec<StoredInstallation>) = query_records(
        transaction,
        "SELECT i.created_at AS installed_at, v.checksum AS schema_checksum, v.applied_at AS schema_applied_at \
         FROM sqb_installation i JOIN sqb_schema_version v ON v.singleton=i.singleton WHERE i.singleton=1",
        Vec::new(),
    )
    .await?;
    commit(transaction).await?;
    let stored = rows.into_iter().next().ok_or_else(|| {
        ControlError::Invalid("the control store has no installation record".into())
    })?;
    let storage: StorageKind = match store {
        ControlStore::Sqlite(_) => StorageKind::Sqlite,
        ControlStore::Postgres(_) => StorageKind::Postgresql,
    };
    Ok(InstallationRecord {
        storage,
        installed_at: stored.installed_at,
        schema_checksum: stored.schema_checksum,
        schema_applied_at: stored.schema_applied_at,
    })
}
