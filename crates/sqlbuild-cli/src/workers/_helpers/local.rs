//! Linux dev workers inherit a claim lock before exec; recovery never guesses from a stale PID.

use std::fs::OpenOptions;
use std::os::unix::fs::MetadataExt;

use command_fds::{CommandFdExt, FdMapping};
use rustix::fs::{FlockOperation, flock};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use sqlbuild_core::command_output::constants::INVOCATION_CONTEXT_ENV;
use sqlbuild_orchestration::runs::models::RunClaim;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::process::Command;

use crate::workers::constants::LOCAL_CLAIM_FD;
use crate::workers::models::{LocalLauncher, LocalWorker, WorkerExit};

impl LocalWorker {
    pub(in crate::workers) fn open(
        location: &LocalLauncher,
        claim: &RunClaim,
    ) -> Result<Self, ControlError> {
        let directory = location.project_dir.join(".sqlbuild/workers");
        std::fs::create_dir_all(&directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!("{}.lock", claim.claim_id)))?;
        Ok(Self {
            lock,
            run_id: claim.request.run_id,
            stopped: false,
        })
    }

    /// Keep the acquired lock until terminal publication, so a delayed spawn cannot start afterward.
    pub(in crate::workers) fn try_wait(&mut self) -> Result<Option<WorkerExit>, ControlError> {
        if !self.stopped {
            match flock(&self.lock, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => self.stopped = true,
                Err(rustix::io::Errno::WOULDBLOCK) => return Ok(None),
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Ok(Some(WorkerExit::Absent))
    }

    /// Only a kernel handle whose current descriptor and invocation match this claim can be signalled.
    pub(in crate::workers) fn signal(&self, signal: Signal) -> Result<(), ControlError> {
        let expected = self.lock.metadata()?;
        for entry in std::fs::read_dir("/proc")? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let raw = match name.parse::<i32>() {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            let Some(pid) = Pid::from_raw(raw) else {
                continue;
            };
            let descriptor = entry.path().join(format!("fd/{LOCAL_CLAIM_FD}"));
            if !same_file(&descriptor, &expected) {
                continue;
            }
            let handle = match pidfd_open(pid, PidfdFlags::empty()) {
                Ok(handle) => handle,
                Err(rustix::io::Errno::SRCH) => continue,
                Err(error) => return Err(std::io::Error::from(error).into()),
            };
            if !same_file(&descriptor, &expected) || !self.owns_invocation(&entry.path())? {
                continue;
            }
            match pidfd_send_signal(handle, signal) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Ok(())
    }

    fn owns_invocation(&self, process: &std::path::Path) -> Result<bool, ControlError> {
        let environment = match std::fs::read(process.join("environ")) {
            Ok(environment) => environment,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let prefix = format!("{INVOCATION_CONTEXT_ENV}=");
        for variable in environment.split(|byte| *byte == 0) {
            if let Some(context) = variable.strip_prefix(prefix.as_bytes()) {
                let context: serde_json::Value =
                    serde_json::from_slice(context).map_err(|error| {
                        ControlError::Invalid(format!(
                            "invalid local worker invocation context: {error}"
                        ))
                    })?;
                return Ok(context["run_id"].as_str() == Some(&self.run_id.to_string()));
            }
        }
        Ok(false)
    }
}

fn same_file(path: &std::path::Path, expected: &std::fs::Metadata) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|actual| actual.dev() == expected.dev() && actual.ino() == expected.ino())
}

/// The child inherits the lock even before Python starts or a worker handshake can succeed.
pub(in crate::workers) fn inherit_claim(
    command: &mut Command,
    location: &LocalLauncher,
    claim: &RunClaim,
) -> Result<(), ControlError> {
    let owner = LocalWorker::open(location, claim)?;
    flock(&owner.lock, FlockOperation::NonBlockingLockExclusive).map_err(std::io::Error::from)?;
    command
        .as_std_mut()
        .fd_mappings(vec![FdMapping {
            parent_fd: owner.lock.into(),
            child_fd: LOCAL_CLAIM_FD,
        }])
        .map_err(|error| ControlError::Invalid(format!("local claim descriptor: {error}")))?;
    Ok(())
}
