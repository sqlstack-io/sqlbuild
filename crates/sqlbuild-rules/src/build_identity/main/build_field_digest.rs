//! BLAKE3 over length-prefixed fields keyed on a cache version and this build, so no two field lists share a key.

use crate::build_identity::main::native_build_identity::native_build_identity;

/// The hex digest of `version`, the native build, `kind` and each field in order.
#[must_use]
pub fn build_field_digest(version: &str, kind: &str, fields: &[&str]) -> String {
    let mut digest = blake3::Hasher::new();
    for field in [version, native_build_identity(), kind]
        .iter()
        .chain(fields.iter())
    {
        digest.update(&(field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    digest.finalize().to_hex().to_string()
}
