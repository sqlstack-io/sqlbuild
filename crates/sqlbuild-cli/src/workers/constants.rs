//! Worker supervision limits.

/// Immutable Docker image identities use the full SHA-256 hexadecimal digest.
pub(super) const IMAGE_ID_HEX_LENGTH: usize = 64;

/// Healthy scheduler cycles take tens of milliseconds; slower ones are logged with phase timings.
pub(super) const SLOW_SCHEDULER_CYCLE: std::time::Duration = std::time::Duration::from_millis(500);
