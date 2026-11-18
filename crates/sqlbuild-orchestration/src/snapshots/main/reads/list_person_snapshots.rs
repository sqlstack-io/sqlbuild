//! A person browses a target's publications, with their connection settings, only with view on every current node, like its jobs.

use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::authorization::main::roles::person_assigns_roles::person_assigns_roles;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::_helpers::published_listing::{
    published_changes, published_connections, published_snapshots,
};
use crate::snapshots::constants::CONNECTION_DOCUMENT;
use crate::snapshots::models::PublishedSnapshot;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Newest first, at most 100 publications; changed roles are named only for people who assign roles.
pub async fn list_person_snapshots(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), limit): ((Uuid, &str), u32),
    default_role: DefaultRole,
) -> Result<Vec<PublishedSnapshot>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let scope = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Integer(i64::from(limit.min(100))),
    ];
    let (transaction, published) = query(transaction,
        "SELECT snapshot_id, generation, git_revision, CASE WHEN git_dirty THEN 1 ELSE 0 END AS git_dirty, git_author, environment_hash, sqb_version, published_at \
         FROM sqb_snapshots WHERE location_id=$1 AND target=$2 ORDER BY generation DESC LIMIT $3",
        scope.clone()).await?;
    let (transaction, changed) = query(transaction,
        "SELECT c.snapshot_id, c.change_kind, c.subject_key FROM sqb_snapshot_changes c \
         WHERE c.snapshot_id IN (SELECT s.snapshot_id FROM sqb_snapshots s WHERE s.location_id=$1 AND s.target=$2 \
           ORDER BY s.generation DESC LIMIT $3) ORDER BY c.snapshot_id, c.change_seq",
        scope.clone()).await?;
    let (transaction, connected) = query(transaction,
        &format!("SELECT d.snapshot_id, b.body FROM sqb_snapshot_documents d JOIN sqb_documents b \
         ON b.document_hash=d.document_hash WHERE d.kind='{CONNECTION_DOCUMENT}' AND d.snapshot_id IN \
         (SELECT s.snapshot_id FROM sqb_snapshots s WHERE s.location_id=$1 AND s.target=$2 \
           ORDER BY s.generation DESC LIMIT $3)"),
        scope).await?;
    let (transaction, names_roles) = person_assigns_roles(transaction, credentials).await?;
    commit(transaction).await?;
    published_snapshots(
        published,
        published_changes(changed, names_roles)?,
        published_connections(connected)?,
    )
}
