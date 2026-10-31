//! Launchers share physical exit observation and termination, not durable lifecycle authority.

use sqlbuild_orchestration::store::errors::ControlError;

use crate::workers::_helpers::supervisor::{exit_code, terminate};
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
            Self::Local(child) => terminate(child),
            Self::Docker(child) => child.signal("SIGTERM").await,
        }
    }

    pub(in crate::workers) async fn kill(&mut self) -> Result<(), ControlError> {
        match self {
            Self::Local(child) => {
                child.start_kill()?;
                Ok(())
            }
            Self::Docker(child) => child.signal("SIGKILL").await,
        }
    }
}
