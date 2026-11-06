//! Stop a recovered worker, and only processes proven to own its claim.

use crate::ownership::models::RecoveredOwnership;

/// `force` sends SIGKILL instead of SIGTERM on Unix; Windows always terminates the claim's job.
pub fn stop_worker(owner: &RecoveredOwnership, force: bool) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    return crate::ownership::_helpers::linux::signal_owners(owner, force);
    #[cfg(target_os = "macos")]
    return crate::ownership::_helpers::macos::signal_owners(owner, force);
    #[cfg(windows)]
    {
        let _ = force;
        owner
            .job
            .as_ref()
            .map_or(Ok(()), crate::ownership::_helpers::windows::terminate)
    }
}
