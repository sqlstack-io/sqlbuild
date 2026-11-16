//! Hexadecimal digest of length-prefixed UTF-8 `parts`, the digest every native cache key uses.

use crate::cache::_helpers::native_store;

/// Hexadecimal digest of length-prefixed UTF-8 `parts`, the digest every native cache key uses.
#[inline]
pub fn content_digest(parts: Vec<String>) -> String {
    native_store::content_digest(parts)
}
