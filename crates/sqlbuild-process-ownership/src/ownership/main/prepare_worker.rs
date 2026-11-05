//! Attach claim ownership to a worker command before it is spawned.

use std::process::Command;

use crate::ownership::constants::WORKER_CLAIM_ENV;
use crate::ownership::models::{Claim, PendingOwnership};

/// Unix children inherit a locked claim descriptor; Windows children start suspended for their job.
pub fn prepare_worker(
    mut command: Command,
    claim: &Claim<'_>,
) -> std::io::Result<(Command, PendingOwnership)> {
    command.env(WORKER_CLAIM_ENV, claim.claim_id.to_string());
    #[cfg(unix)]
    {
        let command = crate::ownership::_helpers::lock::inherit_lock(command, claim)?;
        Ok((command, PendingOwnership {}))
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let job = crate::ownership::_helpers::windows::create_job(claim)?;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_SUSPENDED);
        Ok((command, PendingOwnership { job }))
    }
}
