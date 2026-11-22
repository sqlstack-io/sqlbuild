//! Worker supervision limits.

/// Immutable Docker image identities use the full SHA-256 hexadecimal digest.
pub(super) const IMAGE_ID_HEX_LENGTH: usize = 64;

/// A scheduler cycle longer than this, half the lease, is a real overrun and is logged with its phase timings.
pub(super) const SLOW_SCHEDULER_CYCLE: std::time::Duration = std::time::Duration::from_secs(
    sqlbuild_orchestration::scheduler::constants::LEASE_TTL_SECONDS.unsigned_abs() / 2,
);
