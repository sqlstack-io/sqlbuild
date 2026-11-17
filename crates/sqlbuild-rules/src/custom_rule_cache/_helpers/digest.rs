//! BLAKE3 over length-prefixed fields, so no two field lists share an identity.

use crate::build_identity::main::build_field_digest::build_field_digest;
use crate::custom_rule_cache::constants::CACHE_VERSION;

/// The hex digest of the cache version, the native build and each field in order.
pub(crate) fn field_digest(kind: &str, fields: &[&str]) -> String {
    build_field_digest(CACHE_VERSION, kind, fields)
}

/// An optional field as a presence flag and its text.
pub(crate) fn optional_fields(value: Option<&str>) -> [&str; 2] {
    match value {
        Some(text) => ["1", text],
        None => ["0", ""],
    }
}
