//! Observe whether a recovered worker has exited.

use crate::ownership::models::RecoveredOwnership;

/// A Unix scheduler that sees the exit then holds the lock, so no delayed start can follow.
pub fn worker_exited(owner: &RecoveredOwnership) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use rustix::fs::{FlockOperation, flock};

        match flock(&owner.lock, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(true),
            Err(rustix::io::Errno::WOULDBLOCK) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(windows)]
    {
        match owner.job.as_ref() {
            Some(job) => Ok(crate::ownership::_helpers::windows::active_processes(job)? == 0),
            None => Ok(true),
        }
    }
}
