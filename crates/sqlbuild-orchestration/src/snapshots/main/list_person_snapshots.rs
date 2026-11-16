//! A person browses a target's publications, with their connection settings, only with view on every current node, like its jobs.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::person_assigns_roles::person_assigns_roles;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::constants::CONNECTION_DOCUMENT;
use crate::snapshots::models::{ConnectionDetails, PublishedChange, PublishedSnapshot};
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, decode_row::decode_row, decompress::decompress, query::query,
};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

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
    let mut changes: BTreeMap<Uuid, Vec<PublishedChange>> = BTreeMap::new();
    for row in changed {
        let snapshot_id = id_of(&row)?;
        let mut change: PublishedChange = decode_row(row)?;
        if !names_roles && change.subject_key.starts_with("role:") {
            change.subject_key = "role:".into();
        }
        changes.entry(snapshot_id).or_default().push(change);
    }
    let mut connections: BTreeMap<Uuid, ConnectionDetails> = BTreeMap::new();
    for row in connected {
        let details = serde_json::from_slice(&decompress(&row["body"])?)
            .map_err(|error| ControlError::Invalid(format!("connection document: {error}")))?;
        connections.insert(id_of(&row)?, details);
    }
    published
        .into_iter()
        .map(|row| {
            let mut snapshot = snapshot_of(row)?;
            snapshot.changes = changes.remove(&snapshot.snapshot_id).unwrap_or_default();
            snapshot.connection = connections.remove(&snapshot.snapshot_id);
            Ok(snapshot)
        })
        .collect()
}

fn snapshot_of(row: Row) -> Result<PublishedSnapshot, ControlError> {
    let snapshot_id = id_of(&row)?;
    let text = |key: &str| row[key].as_str().map(String::from);
    let published_at: DateTime<Utc> = serde_json::from_value(row["published_at"].clone())
        .map_err(|error| ControlError::Invalid(format!("stored publication time: {error}")))?;
    Ok(PublishedSnapshot {
        snapshot_id,
        generation: row["generation"].as_i64().unwrap_or_default(),
        git_revision: text("git_revision"),
        git_dirty: row["git_dirty"].as_i64() == Some(1),
        git_author: text("git_author"),
        environment_hash: text("environment_hash").unwrap_or_default(),
        sqb_version: text("sqb_version").unwrap_or_default(),
        published_at,
        changes: Vec::new(),
        connection: None,
    })
}

fn id_of(row: &Row) -> Result<Uuid, ControlError> {
    serde_json::from_value(row["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(format!("stored snapshot id: {error}")))
}
