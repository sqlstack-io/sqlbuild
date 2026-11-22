//! Signal only a child whose process handle is still owned by the caller, with the process group it leads.

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::process::Child;

/// SIGTERM the local worker's process group, or SIGKILL it when `kill`; a worker that already exited is fine.
pub(crate) fn terminate_child(child: &mut Child, kill: bool) -> Result<(), ControlError> {
    #[cfg(unix)]
    if let Some(id) = child.id() {
        let raw = i32::try_from(id)
            .map_err(|error| ControlError::Invalid(format!("invalid child process ID: {error}")))?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| ControlError::Invalid("invalid child process ID".into()))?;
        let signal = if kill {
            rustix::process::Signal::KILL
        } else {
            rustix::process::Signal::TERM
        };
        match rustix::process::kill_process_group(pid, signal) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    #[cfg(not(unix))]
    let _ = (child, kill);
    Ok(())
}
