//! Launchers share physical exit observation and termination, not durable lifecycle authority.

use sqlbuild_orchestration::store::errors::ControlError;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use sqlbuild_process_ownership::ownership::main::{
    stop_worker::stop_worker, worker_exited::worker_exited,
};

use crate::workers::_helpers::supervisor::exit_code;
use crate::workers::main::terminate_child::terminate_child;
use crate::workers::models::{WorkerChild, WorkerExit};

impl WorkerExit {
    pub(in crate::workers) const fn code(self) -> Option<i64> {
        match self {
            Self::Code(code) => Some(code),
            Self::Absent => None,
        }
    }
}

impl WorkerChild {
    pub(in crate::workers) async fn try_wait(
        &mut self,
    ) -> Result<Option<WorkerExit>, ControlError> {
        match self {
            Self::Local(child) => Ok(child
                .try_wait()?
                .map(|exit| WorkerExit::Code(exit_code(exit)))),
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            Self::RecoveredLocal(owner) => Ok(worker_exited(owner)?.then_some(WorkerExit::Absent)),
            Self::Docker(child) => child.try_wait().await,
        }
    }

    pub(in crate::workers) async fn wait(&mut self) -> Result<WorkerExit, ControlError> {
        if let Self::Local(child) = self {
            return Ok(WorkerExit::Code(exit_code(child.wait().await?)));
        }
        loop {
            if let Some(exit) = self.try_wait().await? {
                return Ok(exit);
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    pub(in crate::workers) async fn terminate(&mut self) -> Result<(), ControlError> {
        match self {
            Self::Local(child) => terminate_child(child),
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            Self::RecoveredLocal(owner) => Ok(stop_worker(owner, false)?),
            Self::Docker(child) => child.signal("SIGTERM").await,
        }
    }

    pub(in crate::workers) async fn kill(&mut self) -> Result<(), ControlError> {
        match self {
            Self::Local(child) => {
                child.start_kill()?;
                Ok(())
            }
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            Self::RecoveredLocal(owner) => Ok(stop_worker(owner, true)?),
            Self::Docker(child) => child.signal("SIGKILL").await,
        }
    }
}
