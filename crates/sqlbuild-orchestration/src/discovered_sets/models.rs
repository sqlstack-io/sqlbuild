//! A discovered set as a discover function returns it and as the snapshot keeps it.

use std::collections::{BTreeMap, BTreeSet};

use crate::discovered_sets::types::DiscoveredSets;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One discover function's set: its newest attempt, and the last result that succeeded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredSet {
    pub status: DiscoveredSetStatus,
    /// When the newest attempt ran; `None` for a set no attempt reached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<DateTime<Utc>>,
    /// When `result` was fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_at: Option<DateTime<Utc>>,
    /// The digest of `result`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Why the newest attempt failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How often the location checks the set for changes; `None` checks only when it publishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_every_seconds: Option<u64>,
    /// The last result that succeeded; `None` for a set that never did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<DiscoveredResult>,
}

/// How a set's newest attempt went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredSetStatus {
    /// The newest attempt succeeded.
    Ok,
    /// The newest attempt failed; `result`, if any, is the last good one.
    Failed,
    /// No attempt has run, such as a compile outside a location.
    Never,
}

/// What one discover function returned: its items, and set-wide tags and parallelism.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredResult {
    pub items: Vec<DiscoveredItem>,
    #[serde(default)]
    pub tags: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parallel: Option<u32>,
}

/// One item: its ID, what it waits on, and the arguments its nodes are called with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredItem {
    pub id: String,
    #[serde(default)]
    pub depends_on: Vec<ItemDependency>,
    #[serde(default)]
    pub args: BTreeMap<String, Value>,
}

/// What an item waits on: another item of its set, or a node of the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemDependency {
    Item(String),
    Model(String),
    Source(String),
    Seed(String),
    Job(NodeSetDependency),
    Tag(NodeSetDependency),
}

/// A `job(...)` or `tag(...)` dependency of an item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSetDependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<String>,
}

/// What a location asks its runtime interpreter to publish, and which sets to discover again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryPublishRequest {
    pub generation: i64,
    /// The discover functions to call again; `None` calls every one.
    #[serde(default)]
    pub rediscover: Option<BTreeSet<String>>,
    /// The sets of the last publication, reused for every function not called again.
    #[serde(default)]
    pub previous: DiscoveredSets,
    /// Whether a set that fails with no result of its own reads its last good one from the server.
    #[serde(default)]
    pub seed: bool,
}

/// A location's `check_every` request: call one discover function again without publishing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredSetCheck {
    pub check_discovered: String,
    #[serde(default)]
    pub previous: Option<DiscoveredSet>,
}

/// The runtime's reply to a check: the set after the call, and whether its result changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredSetChecked {
    /// `None` when the runtime holds no compiled project, or the function no longer exists.
    pub set: Option<DiscoveredSet>,
    pub changed: bool,
}
