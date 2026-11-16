//! The warehouse facts a plan reads, as received and as indexed for lookups.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The schema holding a set of state tables; a missing database matches any database.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateLocation {
    pub database: Option<String>,
    pub schema: String,
}

/// One column of a relation, in warehouse order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateColumn {
    pub name: String,
    pub data_type: String,
}

/// One relation of a listed schema, with its columns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRelation {
    pub database: Option<String>,
    pub schema: String,
    pub name: String,
    pub relation_type: String,
    pub columns: Vec<StateColumn>,
}

/// The newest recorded fingerprint of one node identity in one state location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateFingerprint {
    pub location: StateLocation,
    pub node_type: String,
    pub node_name: String,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub run_id: String,
    pub definition_hash: String,
    pub version_hash: String,
    pub schema_fingerprint: String,
    pub definition: String,
    pub metadata_json: String,
    pub ts: DateTime<Utc>,
}

/// The run of a node's newest succeeded build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateLatestRun {
    pub location: StateLocation,
    pub node_type: String,
    pub node_name: String,
    pub run_id: String,
}

/// A node's newest observation of one kind, such as a cursor `MAX` or described source columns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateObservation {
    pub location: StateLocation,
    pub kind: String,
    pub node: String,
    pub cursor_type: Option<String>,
    pub value: String,
    pub run_id: String,
    pub observed_at: DateTime<Utc>,
}

/// One consumption row as the state table holds it; text columns may be NULL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateConsumption {
    pub location: StateLocation,
    pub execution_id: Option<String>,
    pub phase: Option<String>,
    pub run_id: Option<String>,
    pub node_type: Option<String>,
    pub node_name: Option<String>,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub version_hash: Option<String>,
    pub input_count: Option<i64>,
    pub input_kind: Option<String>,
    pub input_name: Option<String>,
    pub input_version: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

/// Where the facts come from: a synced copy of the warehouse state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedProvenance {
    /// When every required state table last synced cleanly; `None` before that.
    pub synced_at: Option<DateTime<Utc>>,
    /// The mirror version the copy was read at; `None` from a server without versions.
    pub version: Option<String>,
}

/// The facts as received, in source order: what builders produce and fixtures store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarehouseStateParts {
    pub provenance: SyncedProvenance,
    pub relations: Vec<StateRelation>,
    pub fingerprints: Vec<StateFingerprint>,
    pub latest_runs: Vec<StateLatestRun>,
    pub observations: Vec<StateObservation>,
    /// Empty unless the copy was read with consumption history.
    pub consumption: Vec<StateConsumption>,
    /// Whether `consumption` holds the history, rather than the copy leaving it out.
    pub has_consumption: bool,
}

/// The facts indexed by name; every lookup is a hash lookup over the folded identifier.
#[derive(Clone, Debug)]
pub struct WarehouseState {
    pub(crate) parts: WarehouseStateParts,
    /// Relation positions by folded name, in source order.
    pub(crate) relations_by_name: HashMap<String, Vec<usize>>,
    /// Fingerprint positions by folded state schema, in source order.
    pub(crate) fingerprints_by_schema: HashMap<String, Vec<usize>>,
    /// Latest-run positions by model name, in source order.
    pub(crate) latest_model_runs: HashMap<String, Vec<usize>>,
    /// Observation positions by kind and node, in source order.
    pub(crate) observations_by_node: HashMap<(String, String), Vec<usize>>,
    /// Consumption positions by folded state schema, in source order.
    pub(crate) consumption_by_schema: HashMap<String, Vec<usize>>,
}

/// What a listing names; each unset part matches every relation.
#[derive(Clone, Copy, Debug)]
pub struct RelationFilter<'filter> {
    pub database: Option<&'filter str>,
    pub schemas: Option<&'filter [String]>,
    pub names: Option<&'filter [String]>,
}

/// A source's described columns, keyed to the SQL that was described.
#[derive(Debug, Deserialize)]
pub(crate) struct DescribedColumns {
    pub(crate) sql_hash: String,
    pub(crate) columns: Vec<String>,
}

/// A stored `source_columns` observation that is not valid JSON of the expected shape.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidObservation {
    pub source: String,
    pub reason: String,
}
