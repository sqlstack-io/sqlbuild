//! `parse_rule_cache`: the usable entries of a stored SQL rule cache.

use crate::build_identity::main::native_build_identity::native_build_identity;
use serde_json::{Map, Value};

use crate::rule_cache::constants::{
    BUILD_IDENTITY_KEY, CACHE_VERSION, ENTRIES_KEY, FINDINGS_KEY, IDENTITY_KEY, VERSION_KEY,
};
use crate::rule_cache::models::CachedEntry;
use crate::rule_cache::types::CachedEntries;

fn entry(value: &Value) -> Option<CachedEntry> {
    let object: &Map<String, Value> = value.as_object()?;
    Some(CachedEntry {
        identity: object.get(IDENTITY_KEY)?.as_str()?.to_owned(),
        findings: object.get(FINDINGS_KEY)?.as_array()?.clone(),
    })
}

/// Entries of a cache written by this format and native build; anything else reads as empty.
#[must_use]
pub fn parse_rule_cache(text: &str) -> CachedEntries {
    let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(text) else {
        return CachedEntries::new();
    };
    let current: bool = payload.get(VERSION_KEY).and_then(Value::as_str) == Some(CACHE_VERSION)
        && payload.get(BUILD_IDENTITY_KEY).and_then(Value::as_str) == Some(native_build_identity());
    let Some(Value::Object(entries)) = payload.get(ENTRIES_KEY).filter(|_| current) else {
        return CachedEntries::new();
    };
    entries
        .iter()
        .filter_map(|(path, value)| Some((path.clone(), entry(value)?)))
        .collect()
}
