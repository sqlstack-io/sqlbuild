//! Rebuild a worker's ownership after the scheduler that launched it stopped.

use crate::ownership::models::{Claim, RecoveredOwnership};

/// Never relies on a remembered process ID.
pub fn recover_worker(claim: &Claim<'_>) -> std::io::Result<RecoveredOwnership> {
    #[cfg(unix)]
    {
        Ok(RecoveredOwnership {
            lock: crate::ownership::_helpers::lock::open_lock(claim)?,
            claim_id: claim.claim_id,
        })
    }
    #[cfg(windows)]
    {
        Ok(RecoveredOwnership {
            job: crate::ownership::_helpers::windows::open_job(claim)?,
        })
    }
}
