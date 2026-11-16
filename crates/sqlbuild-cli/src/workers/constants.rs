//! Worker supervision limits.

/// Immutable Docker image identities use the full SHA-256 hexadecimal digest.
pub(super) const IMAGE_ID_HEX_LENGTH: usize = 64;

/// A cycle that outlasts the lease renewal interval is logged with phase timings; shorter waits for
/// the store's write lock, such as behind a snapshot publication, are harmless and not logged.
pub(super) const SLOW_SCHEDULER_CYCLE: std::time::Duration = std::time::Duration::from_secs(
    sqlbuild_orchestration::scheduler::constants::RENEW_INTERVAL_SECONDS,
);
