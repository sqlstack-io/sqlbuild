//! The digest shared by every migration fingerprint cache key of one build.

use crate::planner_warehouse::constants::{
    FINGERPRINT_CACHE_VERSION, MIGRATION_FINGERPRINT_ALGORITHM,
};
use sha2::{Digest, Sha256};
use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonInteger, JsonValue, StdlibJsonOptions};

/// SHA-256 of the sorted JSON of the algorithm, cache version and SQLBuild version.
pub fn fingerprint_cache_shared_digest(sqlbuild_version: &str) -> Result<String, JsonEmitError> {
    let value = JsonValue::Object(vec![
        (
            "algorithm".to_owned(),
            JsonValue::String(MIGRATION_FINGERPRINT_ALGORITHM.to_owned()),
        ),
        (
            "cache_version".to_owned(),
            JsonValue::Integer(JsonInteger::from(FINGERPRINT_CACHE_VERSION)),
        ),
        (
            "sqlbuild_version".to_owned(),
            JsonValue::String(sqlbuild_version.to_owned()),
        ),
    ]);
    let options = StdlibJsonOptions {
        sort_keys: true,
        ..StdlibJsonOptions::new(None)
    };
    let text = dumps(&value, &JsonDialect::Stdlib(options))?;
    Ok(format!("{:x}", Sha256::digest(text.as_bytes())))
}
