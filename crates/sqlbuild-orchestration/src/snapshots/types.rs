//! Shared forms of snapshot data.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::snapshots::models::SnapshotNode;

/// A snapshot's nodes, decoded once and shared by every reader of the snapshot.
pub(crate) type SnapshotNodes = Arc<BTreeMap<String, SnapshotNode>>;
