//! Messages of the build identity.

/// Why reading the identity before the binary installed it fails.
pub(crate) const MISSING_BUILD_IDENTITY: &str = "the native build identity was not installed: the binary must call install_build_identity at startup, before any cache key or rule fingerprint is read";
