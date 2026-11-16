//! BLAKE3 over length-prefixed fields, so no two field lists share a key.

use crate::build_identity::main::build_field_digest::build_field_digest;
use crate::rule_cache::constants::CACHE_VERSION;

/// The hex digest of the cache version, the native build and each field in order.
pub(crate) fn field_digest(kind: &str, fields: &[&str]) -> String {
    build_field_digest(CACHE_VERSION, kind, fields)
}
