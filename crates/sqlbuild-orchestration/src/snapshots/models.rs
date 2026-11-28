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

/// Decoded snapshots by ID, and the memory budget their decoding shares.
#[derive(Debug)]
pub(crate) struct SnapshotNodeCache {
    pub(crate) slots: std::sync::Mutex<SnapshotCells<(), crate::snapshots::types::SnapshotNodes>>,
    /// One permit per MiB a read may decode into; a read waits for its estimate, capped at the whole budget.
    pub(crate) budget: tokio::sync::Semaphore,
}

/// Values cached once per snapshot, each in a scope such as a location target; a larger `used` is more recent.
#[derive(Debug)]
pub(crate) struct SnapshotCells<S, T> {
    /// Counts lookups, so a larger `used` is more recent.
    pub(crate) clock: u64,
    pub(crate) slots: Vec<SnapshotCell<S, T>>,
}

/// One snapshot's value, made by the first reader that needs it while later ones wait.
#[derive(Debug)]
pub(crate) struct SnapshotCell<S, T> {
    pub(crate) scope: S,
    pub(crate) snapshot_id: Uuid,
    pub(crate) used: u64,
    pub(crate) cell: std::sync::Arc<tokio::sync::OnceCell<T>>,
}

impl<S, T> SnapshotCells<S, T> {
    pub(crate) const fn new() -> Self {
        Self {
            clock: 0,
            slots: Vec::new(),
        }
    }
}

impl SnapshotNodeCache {
    pub(crate) const fn new() -> Self {
        Self {
            slots: std::sync::Mutex::new(SnapshotCells::new()),
            budget: tokio::sync::Semaphore::const_new(
                crate::snapshots::constants::SNAPSHOT_DECODE_BUDGET_MIB as usize,
            ),
        }
    }
}

/// SQL inspection never returns Python source, execution artifacts or unrelated node metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeSql {
    pub snapshot_id: Uuid,
    pub node_key: String,
    pub raw_sql: Option<String>,
    pub compiled_sql: Option<String>,
    /// Each placed column's defining expression as a `[start, end)` byte range of `compiled_sql`.
    #[serde(default)]
    pub expression_ranges: BTreeMap<String, [usize; 2]>,
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

/// What a published node contributes to a write set, without the rest of its definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriterNode {
    pub(crate) kind: String,
    /// The `source:` keys among the node's compiled `deps`.
    pub(crate) source_deps: Vec<String>,
    /// The keys the definition lists in `loads`; snapshots published before it have none.
    pub(crate) loads: Vec<String>,
    /// A source with a non-null loader, which a loading build writes.
    pub(crate) managed: bool,
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

/// Installation configuration: how many recent publications to keep per location and target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotRetention {
    count: u16,
}

impl SnapshotRetention {
    pub fn count(count: u16) -> Result<Self, crate::store::errors::ControlError> {
        if count == 0 {
            return Err(crate::store::errors::ControlError::Invalid(
                "snapshot retention must keep at least one snapshot".into(),
            ));
        }
        Ok(Self { count })
    }

    pub(crate) fn kept(self) -> i64 {
        i64::from(self.count)
    }

    pub(crate) fn kept_count(self) -> u16 {
        self.count
    }
}

impl Default for SnapshotRetention {
    fn default() -> Self {
        Self {
            count: crate::snapshots::constants::DEFAULT_SNAPSHOT_RETENTION_COUNT,
        }
    }
}

/// One publication as people browse it: its commit, environment and changed definitions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct PublishedSnapshot {
    pub snapshot_id: Uuid,
    pub generation: i64,
    pub git_revision: Option<String>,
    pub git_dirty: bool,
    pub git_author: Option<String>,
    /// Digest of the lock file, Python and `sqb` versions; a change means dependencies changed.
    pub environment_hash: String,
    pub sqb_version: String,
    pub published_at: DateTime<Utc>,
    /// Added, changed and removed nodes and roles, in publication order.
    pub changes: Vec<PublishedChange>,
    /// The target's non-secret connection settings; null when the location published none.
    pub connection: Option<ConnectionDetails>,
}

/// A target's connection settings as its location publishes them: names only, never a secret.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConnectionDetails {
    /// The adapter, such as `snowflake`, `postgres` or `duckdb`.
    pub adapter: String,
    pub account: Option<String>,
    /// The server host, or for a file database its path.
    pub host: Option<String>,
    pub user: Option<String>,
    pub role: Option<String>,
    pub warehouse: Option<String>,
    /// The database the connection opens by default.
    pub database: Option<String>,
    /// How the location signs in, by name only, such as `key_pair`, `password` or `oauth`.
    pub auth_method: Option<String>,
}

/// A changed subject, such as `model:orders`, or `role:analyst` (`role:` for those who can't assign roles).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PublishedChange {
    pub change_kind: String,
    pub subject_key: String,
}
