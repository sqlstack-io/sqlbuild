//! `rule_cache_text`: the stored form of the SQL rule cache.

use crate::build_identity::main::native_build_identity::native_build_identity;
use serde_json::{Map, Value, json};

use crate::rule_cache::constants::{
    BUILD_IDENTITY_KEY, CACHE_VERSION, ENTRIES_KEY, FINDINGS_KEY, IDENTITY_KEY, VERSION_KEY,
};
use crate::rule_cache::types::CachedEntries;

/// Compact JSON of the format version, native build and every entry by path.
#[must_use]
pub fn rule_cache_text(entries: &CachedEntries) -> String {
    let stored: Map<String, Value> = entries
        .iter()
        .map(|(path, entry)| {
            let value = json!({ IDENTITY_KEY: entry.identity, FINDINGS_KEY: entry.findings });
            (path.clone(), value)
        })
        .collect();
    json!({
        VERSION_KEY: CACHE_VERSION,
        BUILD_IDENTITY_KEY: native_build_identity(),
        ENTRIES_KEY: stored,
    })
    .to_string()
}
