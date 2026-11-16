//! The native content digest of artifact bytes, as reuse verifies written artifacts.

use crate::compile_outputs::_helpers::reuse::native_digest;

/// The native content digest of artifact bytes, as reuse verifies written artifacts.
pub fn artifact_digest(contents: &[u8]) -> String {
    native_digest(contents)
}
