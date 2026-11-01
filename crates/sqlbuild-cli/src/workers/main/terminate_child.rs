//! Signal only a child whose process handle is still owned by the caller.

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::process::Child;

pub(crate) fn terminate_child(child: &mut Child) -> Result<(), ControlError> {
    #[cfg(unix)]
    if let Some(id) = child.id() {
        let raw = i32::try_from(id)
            .map_err(|error| ControlError::Invalid(format!("invalid child process ID: {error}")))?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| ControlError::Invalid("invalid child process ID".into()))?;
        match rustix::process::kill_process(pid, rustix::process::Signal::TERM) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    #[cfg(not(unix))]
    let _ = child;
    Ok(())
}
