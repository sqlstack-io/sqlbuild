//! Complete ownership once the prepared command has spawned.

use crate::ownership::models::PendingOwnership;

/// On Windows the suspended child joins its job and only then runs; the caller kills it on error.
pub fn start_worker(pending: PendingOwnership, process_id: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let PendingOwnership {} = pending;
        let _ = process_id;
        Ok(())
    }
    #[cfg(windows)]
    crate::ownership::_helpers::windows::adopt(&pending.job, process_id)
}
