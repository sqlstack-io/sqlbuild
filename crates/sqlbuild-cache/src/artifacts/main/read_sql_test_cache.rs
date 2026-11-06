//! Read the persisted SQL-test artifact records without an interpreter.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::artifacts::_helpers::sql_test_records::{
    is_safe_relative_sql_path, normalized_relative_path,
};
use crate::artifacts::constants::{
    MAX_SQL_TEST_CACHE_BYTES, MAX_SQL_TEST_CACHE_RECORDS, SQL_TEST_CACHE_FILE,
    SQL_TEST_CACHE_VERSION,
};
use crate::artifacts::models::SqlTestArtifactRecord;

/// Valid records in the existing snapshot; an unreadable or malformed cache is a miss.
#[must_use]
pub fn read_sql_test_cache(directory: &Path) -> HashMap<String, SqlTestArtifactRecord> {
    let path = directory.join(SQL_TEST_CACHE_FILE);
    let Ok(metadata) = std::fs::metadata(&path) else {
        return HashMap::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_SQL_TEST_CACHE_BYTES {
        return HashMap::new();
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return HashMap::new();
    };
    let Ok(payload) = serde_json::from_slice::<Value>(&bytes) else {
        return HashMap::new();
    };
    if payload.get("version").and_then(Value::as_f64) != Some(SQL_TEST_CACHE_VERSION as f64) {
        return HashMap::new();
    }
    let Some(rows) = payload.get("records").and_then(Value::as_object) else {
        return HashMap::new();
    };
    if rows.len() > MAX_SQL_TEST_CACHE_RECORDS {
        return HashMap::new();
    }
    let mut records: HashMap<String, SqlTestArtifactRecord> = HashMap::new();
    for (key, value) in rows {
        if let Some(record) = record(value) {
            records.insert(key.clone(), record);
        }
    }
    records
}

fn record(value: &Value) -> Option<SqlTestArtifactRecord> {
    let relative_path = normalized_relative_path(Path::new(value.get("relative_path")?.as_str()?));
    if !is_safe_relative_sql_path(&relative_path) {
        return None;
    }
    let Ok(mtime_ns) = i64::try_from(value.get("mtime_ns")?.as_u64()?) else {
        return None;
    };
    Some(SqlTestArtifactRecord {
        identity: value.get("identity")?.as_str()?.to_owned(),
        relative_path,
        size: value.get("size")?.as_u64()?,
        mtime_ns,
        content_sha256: value.get("content_sha256")?.as_str()?.to_owned(),
    })
}
