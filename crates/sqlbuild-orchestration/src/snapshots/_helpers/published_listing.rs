//! A target's publications as a person browses them, from their stored rows.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::snapshots::models::{ConnectionDetails, PublishedChange, PublishedSnapshot};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, decompress::decompress};
use crate::store::types::Row;

/// Each publication's changes; role names stay hidden from people who cannot assign roles.
pub(crate) fn published_changes(
    rows: Vec<Row>,
    names_roles: bool,
) -> Result<BTreeMap<Uuid, Vec<PublishedChange>>, ControlError> {
    let mut changes: BTreeMap<Uuid, Vec<PublishedChange>> = BTreeMap::new();
    for row in rows {
        let snapshot_id = id_of(&row)?;
        let mut change: PublishedChange = decode_row(row)?;
        if !names_roles && change.subject_key.starts_with("role:") {
            change.subject_key = "role:".into();
        }
        changes.entry(snapshot_id).or_default().push(change);
    }
    Ok(changes)
}

/// Each publication's connection settings, from its stored connection document.
pub(crate) fn published_connections(
    rows: Vec<Row>,
) -> Result<BTreeMap<Uuid, ConnectionDetails>, ControlError> {
    rows.into_iter()
        .map(|row| {
            let details = serde_json::from_slice(&decompress(&row["body"])?)
                .map_err(|error| ControlError::Invalid(format!("connection document: {error}")))?;
            Ok((id_of(&row)?, details))
        })
        .collect()
}

/// The publications, newest first, each with its changes and connection settings.
pub(crate) fn published_snapshots(
    rows: Vec<Row>,
    mut changes: BTreeMap<Uuid, Vec<PublishedChange>>,
    mut connections: BTreeMap<Uuid, ConnectionDetails>,
) -> Result<Vec<PublishedSnapshot>, ControlError> {
    rows.into_iter()
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
