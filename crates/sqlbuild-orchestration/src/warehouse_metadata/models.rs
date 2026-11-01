//! Existing warehouse state rows, with their physical state-table provenance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Preserve observations, not an inferred latest version, age status or run outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseFreshnessRecord {
    pub event_id: Option<String>,
    pub state_database: Option<String>,
    pub state_schema: String,
    pub source_name: String,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub run_id: String,
    pub strategy: String,
    pub value_kind: String,
    pub data_version: Option<String>,
    pub data_version_hash: String,
    pub observed_at: DateTime<Utc>,
}

/// A bounded page from one registered reader of a published location/target generation.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceFreshnessBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseFreshnessRecord>,
}

/// Physical namespace of SQLBuild's source-freshness state table, never a source query.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseStateLocation {
    pub database: Option<String>,
    pub schema: String,
}

/// Private interpreter input; cursor values are data and receive adapter literal rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFreshnessRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 11]>,
    /// Inclusive time bound, held fixed across pages; independent of the row-key cursor.
    pub observed_since: Option<DateTime<Utc>>,
}

/// Fingerprints remain history here; latest-row selection belongs to the existing planner reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseFingerprintRecord {
    pub event_id: Option<String>,
    pub state_database: Option<String>,
    pub state_schema: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 13]>,
    pub observed_since: Option<DateTime<Utc>>,
}

/// The private interpreter accepts only these two known state tables, never arbitrary SQL.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarehouseHistoryRead {
    SourceFreshness(SourceFreshnessRead),
    Fingerprints(FingerprintRead),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WarehouseHistoryRecord {
    SourceFreshness(WarehouseFreshnessRecord),
    Fingerprint(WarehouseFingerprintRecord),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseHistoryPage {
    pub records: Vec<WarehouseHistoryRecord>,
    pub next_after: Option<Vec<Option<String>>>,
}
