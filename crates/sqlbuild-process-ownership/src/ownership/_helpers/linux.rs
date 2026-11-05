//! Linux: signal through a pidfd only after `/proc` shows the claim descriptor and marker.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use crate::ownership::_helpers::lock::carries_claim;
use crate::ownership::constants::CLAIM_FD;
use crate::ownership::models::RecoveredOwnership;

fn same_file(path: &Path, expected: &std::fs::Metadata) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|actual| actual.dev() == expected.dev() && actual.ino() == expected.ino())
}

/// The descriptor is checked again after the pidfd pins the process, so a reused PID is never hit.
pub(crate) fn signal_owners(owner: &RecoveredOwnership, force: bool) -> std::io::Result<()> {
    let signal = if force { Signal::KILL } else { Signal::TERM };
    let expected = owner.lock.metadata()?;
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        let name = entry.file_name();
        let raw = match name.to_str().map(str::parse::<i32>) {
            Some(Ok(raw)) => raw,
            Some(Err(_)) | None => continue,
        };
        let Some(pid) = Pid::from_raw(raw) else {
            continue;
        };
        let descriptor = entry.path().join(format!("fd/{CLAIM_FD}"));
        if !same_file(&descriptor, &expected) {
            continue;
        }
        let handle = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(handle) => handle,
            Err(rustix::io::Errno::SRCH) => continue,
            Err(error) => return Err(error.into()),
        };
        let environment = match std::fs::read(entry.path().join("environ")) {
            Ok(environment) => environment,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !same_file(&descriptor, &expected) || !carries_claim(&environment, owner.claim_id) {
            continue;
        }
        match pidfd_send_signal(handle, signal) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
