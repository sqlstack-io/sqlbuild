//! Statements a dialect's native parser read whole, kept so a warm compile skips parsing them again.

use sqlbuild_analysis::assembly::project::models::SyntaxCheck;
use sqlbuild_cache::digest::main::content_digest::content_digest;
use sqlbuild_cache::digest::types::ContentDigest;

/// The native store kind holding read-whole verdicts.
pub(crate) const NATIVE_PARSE_STORE_KIND: &str = "native-parses";
/// The store file below the compile cache folder.
pub(crate) const NATIVE_PARSE_STORE_FILE_NAME: &str = "native-parses.bin";
/// Bumped whenever what a key covers changes; older stores open empty.
pub(crate) const NATIVE_PARSE_STORE_VERSION: &str = "native-parse-store-v1";

/// The key `dialect`'s verdict on `check` is stored under.
pub(crate) fn parse_key(dialect: &str, check: &SyntaxCheck) -> ContentDigest {
    let mut parts: Vec<&str> = vec![dialect, &check.sql];
    for (name, value) in &check.placeholders {
        parts.push(name);
        parts.push(value);
    }
    content_digest(&parts)
}
