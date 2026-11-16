//! A content hash of every native crate, including the release version in their manifests.

/// The source content hash, never inlined so callers do not change, and lose incremental reuse, after every edit.
#[inline(never)]
#[must_use]
pub fn native_build_identity() -> &'static str {
    env!("SQLBUILD_NATIVE_SOURCE_HASH")
}
