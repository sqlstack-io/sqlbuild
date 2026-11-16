//! Node states, their versioned in-memory views and the state changes pushed on the change stream.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use axum::body::Bytes;
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
}

/// A serialized response and the entity tag naming its snapshot and state version.
#[derive(Clone, Debug)]
pub struct CachedJson {
    pub etag: Option<String>,
    pub body: Bytes,
}

/// Each held target's view, keyed by location and target hash.
pub(crate) type TargetCaches = Arc<Mutex<HashMap<(Uuid, String), Arc<TargetCache>>>>;

/// This server's views, keyed by location and target; clones share them.
#[derive(Clone, Debug)]
pub struct NodeStateCache {
    pub(crate) store: ControlStore,
    pub(crate) targets: TargetCaches,
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
}

/// One version of a target's states; each response body is serialized at most once.
#[derive(Debug)]
pub(crate) struct TargetView {
    pub(crate) version: u64,
    pub(crate) catalog: Arc<SnapshotCatalog>,
    pub(crate) states: Arc<BTreeMap<String, NodeState>>,
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
