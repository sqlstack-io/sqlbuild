//! Publish the existing SQL-test cache snapshot directly from native records.

use std::collections::BTreeMap;
use std::path::Path;

use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonValue, StdlibJsonOptions};

use crate::artifacts::constants::{SQL_TEST_CACHE_FILE, SQL_TEST_CACHE_VERSION};
use crate::artifacts::models::SqlTestArtifactRecord;

/// Atomically replace the snapshot with Python-compatible JSON; write failures are cache misses.
pub fn write_sql_test_cache(directory: &Path, records: &[(String, SqlTestArtifactRecord)]) {
    let rows: BTreeMap<String, JsonValue> = records
        .iter()
        .map(|(key, record)| {
            let fields = JsonValue::Object(vec![
                (
                    "identity".to_owned(),
                    JsonValue::String(record.identity.clone()),
                ),
                (
                    "relative_path".to_owned(),
                    JsonValue::String(record.relative_posix()),
                ),
                ("size".to_owned(), JsonValue::Integer(record.size.into())),
                (
                    "mtime_ns".to_owned(),
                    JsonValue::Integer(record.mtime_ns.into()),
                ),
                (
                    "content_sha256".to_owned(),
                    JsonValue::String(record.content_sha256.clone()),
                ),
            ]);
            (key.clone(), fields)
        })
        .collect();
    let payload = JsonValue::Object(vec![
        (
            "version".to_owned(),
            JsonValue::Integer(SQL_TEST_CACHE_VERSION.into()),
        ),
        (
            "records".to_owned(),
            JsonValue::Object(rows.into_iter().collect()),
        ),
    ]);
    let Ok(encoded) = dumps(
        &payload,
        &JsonDialect::Stdlib(StdlibJsonOptions {
            sort_keys: true,
            ..StdlibJsonOptions::new(None).with_separators(",", ":")
        }),
    ) else {
        return;
    };
    let temporary = directory.join(format!(".{SQL_TEST_CACHE_FILE}.{}.tmp", std::process::id()));
    let _ = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(directory)?;
        std::fs::write(&temporary, encoded.as_bytes())?;
        std::fs::rename(&temporary, directory.join(SQL_TEST_CACHE_FILE))
    })();
}
