//! The SHA-256 hash used for local and composed version identities.

use std::fmt::Write;

use sha2::{Digest, Sha256};

/// The lowercase hex SHA-256 digest of the UTF-8 text.
pub fn stable_identity_hash(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}
