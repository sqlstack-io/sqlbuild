//! The key `dialect`'s verdict on `check` is stored under.

use crate::cache::_helpers::native_parse_store;
use sqlbuild_analysis::assembly::project::models::SyntaxCheck;
use sqlbuild_cache::digest::types::ContentDigest;

/// The key `dialect`'s verdict on `check` is stored under.
#[inline]
pub fn parse_key(dialect: &str, check: &SyntaxCheck) -> ContentDigest {
    native_parse_store::parse_key(dialect, check)
}
