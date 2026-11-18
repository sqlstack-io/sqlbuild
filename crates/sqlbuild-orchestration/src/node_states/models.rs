//! Node states, their versioned in-memory views and the state changes pushed on the change stream.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use axum::body::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::snapshots::models::SnapshotNode;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeChecks, CatalogNodeOutcome, CatalogNodeRecord,
};

/// The state fields of a `CatalogNodeRecord`, with the same meaning.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeState {
    pub last_build: Option<CatalogNodeBuild>,
    pub last_outcome: Option<CatalogNodeOutcome>,
    pub definition_changed: bool,
    pub checks: CatalogNodeChecks,
}

/// Every current node's state at one version; `snapshot_id` is null before the first publication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeStatesRecord {
    pub snapshot_id: Option<Uuid>,
    pub version: u64,
    pub states: BTreeMap<String, NodeState>,
    /// What the default `sqb build --changed` would run; null until the location plans it for the current snapshot.
    #[serde(default)]
    pub changed_build: Option<ChangedBuild>,
}

/// The nodes the default `sqb build --changed` runs, as the location last planned it from the synced copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChangedBuild {
    /// The publication generation it was planned for.
    pub generation: i64,
    /// When the server received it.
    pub reported_at: DateTime<Utc>,
    /// Nodes that run for a change of their own, as `kind:name`.
    pub changed: Vec<String>,
    /// Nodes that run only because an upstream they read runs.
    pub downstream: Vec<String>,
    /// Why the location's plan failed, with both lists empty; absent when it planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing: Option<String>,
}

/// The stored node lists of a changed build, or why its plan failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangedBuildNodes {
    pub(crate) changed: Vec<String>,
    pub(crate) downstream: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) failing: Option<String>,
}

/// A location's plan of the default `sqb build --changed`, fenced to its reader session and generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangedBuildReport {
    pub session_id: Uuid,
    pub generation: i64,
    pub changed: Vec<String>,
    pub downstream: Vec<String>,
    /// Why the plan failed, sent with empty lists; only newer servers accept it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing: Option<String>,
}

/// A serialized response and the entity tag naming its snapshot and state version.
#[derive(Clone, Debug)]
pub struct CachedJson {
    pub etag: Option<String>,
    pub body: Bytes,
}

/// This server's views, keyed by location and target; clones share them.
#[derive(Clone, Debug)]
pub struct NodeStateCache {
    pub(crate) store: ControlStore,
    pub(crate) targets: crate::node_states::types::TargetCaches,
    pub(crate) updates: tokio::sync::broadcast::Sender<NodeStatusUpdate>,
}

/// One target's current view; `refresh` serializes the store reads that replace it.
#[derive(Debug, Default)]
pub(crate) struct TargetCache {
    pub(crate) target_hash: String,
    pub(crate) view: RwLock<Option<Arc<TargetView>>>,
    pub(crate) refresh: tokio::sync::Mutex<()>,
    pub(crate) dirty: AtomicBool,
    pub(crate) scheduled: AtomicBool,
    /// A store signal named this target since its last refresh, so the refresh pushes a notice even when nothing changed.
    pub(crate) announce: AtomicBool,
}

/// One version of a target's states; each response body is serialized at most once.
#[derive(Debug)]
pub(crate) struct TargetView {
    pub(crate) version: u64,
    pub(crate) catalog: Arc<SnapshotCatalog>,
    pub(crate) states: Arc<BTreeMap<String, NodeState>>,
    pub(crate) changed_build: Option<Arc<ChangedBuild>>,
    pub(crate) nodes_body: tokio::sync::OnceCell<Bytes>,
    pub(crate) states_body: tokio::sync::OnceCell<Bytes>,
}

/// A snapshot's node records without their states, and the definition fields the states derive from.
#[derive(Debug, Default)]
pub(crate) struct SnapshotCatalog {
    pub(crate) snapshot_id: Option<Uuid>,
    pub(crate) records: Vec<CatalogNodeRecord>,
    pub(crate) inputs: Vec<(String, SnapshotNode)>,
}

/// One target's serialized `node_status` event data.
#[derive(Clone, Debug)]
pub(crate) struct NodeStatusUpdate {
    pub(crate) location_id: Uuid,
    pub(crate) target_hash: String,
    pub(crate) data: Arc<str>,
}
