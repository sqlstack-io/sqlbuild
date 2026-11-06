//! Fingerprint table column names, in table order.

pub const COLUMN_NODE_TYPE: &str = "node_type";
pub const COLUMN_NODE_NAME: &str = "node_name";
pub const COLUMN_TARGET_DATABASE: &str = "target_database";
pub const COLUMN_TARGET_SCHEMA: &str = "target_schema";
pub const COLUMN_TARGET_NAME: &str = "target_name";
pub const COLUMN_RUN_ID: &str = "run_id";
pub const COLUMN_DEFINITION_HASH: &str = "definition_hash";
pub const COLUMN_VERSION_HASH: &str = "version_hash";
pub const COLUMN_SCHEMA_FINGERPRINT: &str = "schema_fingerprint";
pub const COLUMN_DEFINITION_B64: &str = "definition_b64";
pub const COLUMN_METADATA_JSON_B64: &str = "metadata_json_b64";
pub const COLUMN_TIMESTAMP: &str = "ts";
pub const COLUMN_EVENT_ID: &str = "event_id";

/// Legacy columns used by latest-row readers, including readers of tables without an event ID.
pub const FINGERPRINT_COLUMNS: [&str; 12] = [
    COLUMN_NODE_TYPE,
    COLUMN_NODE_NAME,
    COLUMN_TARGET_DATABASE,
    COLUMN_TARGET_SCHEMA,
    COLUMN_TARGET_NAME,
    COLUMN_RUN_ID,
    COLUMN_DEFINITION_HASH,
    COLUMN_VERSION_HASH,
    COLUMN_SCHEMA_FINGERPRINT,
    COLUMN_DEFINITION_B64,
    COLUMN_METADATA_JSON_B64,
    COLUMN_TIMESTAMP,
];

/// The node type that wins a node-name collision between latest rows.
pub const NODE_TYPE_MODEL: &str = "model";
/// The node type of seed fingerprints.
pub const NODE_TYPE_SEED: &str = "seed";
