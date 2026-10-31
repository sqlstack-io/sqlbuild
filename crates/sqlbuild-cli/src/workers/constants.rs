//! Immutable Docker image identities use the full SHA-256 hexadecimal digest.

pub(super) const IMAGE_ID_HEX_LENGTH: usize = 64;

/// Keep inherited claim ownership away from standard IO and the runtime's usual low descriptors.
#[cfg(target_os = "linux")]
pub(super) const LOCAL_CLAIM_FD: i32 = 198;
