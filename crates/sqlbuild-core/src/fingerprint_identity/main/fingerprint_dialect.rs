//! The fingerprint dialect a project's SQL dialect name maps to.

use crate::fingerprint_identity::constants::{
    FINGERPRINT_DIALECT_ALIASES, GENERIC_FINGERPRINT_DIALECT,
};

/// Python's `compute_query_hash` dialect: the aliased name, or generic when empty or absent.
#[must_use]
pub fn fingerprint_dialect(dialect: Option<&str>) -> &str {
    match dialect.filter(|name| !name.is_empty()) {
        None => GENERIC_FINGERPRINT_DIALECT,
        Some(name) => FINGERPRINT_DIALECT_ALIASES
            .iter()
            .find(|(alias, _)| *alias == name)
            .map_or(name, |(_, target)| target),
    }
}
