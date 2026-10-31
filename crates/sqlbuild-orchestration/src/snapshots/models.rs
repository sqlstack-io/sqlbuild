//! Snapshot inputs are data only: no executable user objects cross into the control plane.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A compiled node and its display-only source. SQL and Python are never run by the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SnapshotNode {
    pub kind: String,
    pub definition: Value,
    pub raw_sql: Option<String>,
    pub compiled_sql: Option<String>,
    pub python_source: Option<String>,
}

/// A location's successful compile for one target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SnapshotInput {
    pub location_id: Uuid,
    pub target: String,
    pub generation: i64,
    pub git_revision: Option<String>,
    pub git_dirty: bool,
    pub git_author: Option<String>,
    pub image_id: Option<String>,
    pub environment_hash: String,
    pub sqb_version: String,
    pub nodes: BTreeMap<String, SnapshotNode>,
    pub documents: BTreeMap<String, Value>,
}

/// Immutable publication metadata. Generations increase independently per location and target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SnapshotRecord {
    pub snapshot_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub generation: i64,
    pub manifest_digest: String,
    pub published_at: DateTime<Utc>,
}

/// Node-level changes do not duplicate unchanged definitions or source text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SnapshotChange {
    pub change_kind: String,
    pub subject_key: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
}

/// The current successful snapshot, read with a consistent generation and change set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CurrentSnapshot {
    pub record: SnapshotRecord,
    pub nodes: BTreeMap<String, SnapshotNode>,
    pub documents: BTreeMap<String, Value>,
    pub changes: Vec<SnapshotChange>,
}

/// Canonical identity is shared by publication and worker verification; only publication compresses.
#[derive(Debug)]
pub(crate) struct NodeContent<'a> {
    pub(crate) definition: Vec<u8>,
    pub(crate) sources: BTreeMap<&'static str, (String, &'a str)>,
}

#[derive(Debug)]
pub(crate) struct PreparedNode {
    pub(crate) kind: String,
    pub(crate) hash: String,
    pub(crate) definition: Vec<u8>,
    pub(crate) sources: BTreeMap<&'static str, (String, Vec<u8>)>,
}

#[derive(Debug)]
pub(crate) struct PreparedSnapshot {
    pub(crate) nodes: BTreeMap<String, PreparedNode>,
    pub(crate) hashes: BTreeMap<String, String>,
    pub(crate) documents: BTreeMap<String, (String, Vec<u8>)>,
    pub(crate) manifest_digest: String,
}
