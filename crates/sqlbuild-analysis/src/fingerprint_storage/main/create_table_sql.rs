//! Fingerprint table DDL.

use crate::fingerprint_storage::constants::{
    COLUMN_DEFINITION_B64, COLUMN_DEFINITION_HASH, COLUMN_EVENT_ID, COLUMN_METADATA_JSON_B64,
    COLUMN_NODE_NAME, COLUMN_NODE_TYPE, COLUMN_RUN_ID, COLUMN_SCHEMA_FINGERPRINT,
    COLUMN_TARGET_DATABASE, COLUMN_TARGET_NAME, COLUMN_TARGET_SCHEMA, COLUMN_TIMESTAMP,
    COLUMN_VERSION_HASH,
};

/// `CREATE TABLE IF NOT EXISTS` for the fingerprint table, with the adapter's column types.
pub fn create_table_sql(qualified_name: &str, string_type: &str, timestamp_type: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {qualified_name} (\
         {COLUMN_NODE_TYPE} {string_type} NOT NULL, \
         {COLUMN_NODE_NAME} {string_type} NOT NULL, \
         {COLUMN_TARGET_DATABASE} {string_type}, \
         {COLUMN_TARGET_SCHEMA} {string_type}, \
         {COLUMN_TARGET_NAME} {string_type}, \
         {COLUMN_RUN_ID} {string_type} NOT NULL, \
         {COLUMN_DEFINITION_HASH} {string_type} NOT NULL, \
         {COLUMN_VERSION_HASH} {string_type} NOT NULL, \
         {COLUMN_SCHEMA_FINGERPRINT} {string_type} NOT NULL, \
         {COLUMN_DEFINITION_B64} {string_type} NOT NULL, \
         {COLUMN_METADATA_JSON_B64} {string_type} NOT NULL, \
         {COLUMN_TIMESTAMP} {timestamp_type} NOT NULL, \
         {COLUMN_EVENT_ID} {string_type})"
    )
}
