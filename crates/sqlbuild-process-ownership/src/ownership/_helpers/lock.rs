//! Unix claim locks: inherited across exec, held exactly as long as some owner lives.

use std::fs::{File, OpenOptions};
use std::process::Command;

use command_fds::{CommandFdExt, FdMapping};
use rustix::fs::{FlockOperation, flock};
use uuid::Uuid;

use crate::ownership::constants::{CLAIM_FD, WORKER_CLAIM_ENV};
use crate::ownership::models::Claim;

pub(crate) fn open_lock(claim: &Claim<'_>) -> std::io::Result<File> {
    let directory = claim.project_dir.join(".sqlbuild/workers");
    std::fs::create_dir_all(&directory)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{}.lock", claim.claim_id)))
}

pub(crate) fn inherit_lock(mut command: Command, claim: &Claim<'_>) -> std::io::Result<Command> {
    let lock = open_lock(claim)?;
    flock(&lock, FlockOperation::NonBlockingLockExclusive)?;
    command
        .fd_mappings(vec![FdMapping {
            parent_fd: lock.into(),
            child_fd: CLAIM_FD,
        }])
        .map_err(|error| std::io::Error::other(format!("claim descriptor: {error}")))?;
    Ok(command)
}

/// `environment` is a process's initial NUL-separated `NAME=value` block.
pub(crate) fn carries_claim(environment: &[u8], claim_id: Uuid) -> bool {
    let expected = format!("{WORKER_CLAIM_ENV}={claim_id}");
    environment
        .split(|byte| *byte == 0)
        .any(|variable| variable == expected.as_bytes())
}
