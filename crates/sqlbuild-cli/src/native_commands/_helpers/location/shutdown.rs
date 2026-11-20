//! Stop on SIGINT, SIGTERM or SIGHUP; during the shutdown that starts, a second signal kills the workers instead of waiting for them.

use tokio::sync::watch;
use tokio::task::JoinSet;

use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Stream;
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::constants::FORCED_WORKER_STOP;
use crate::native_commands::models::ShutdownSignals;

impl ShutdownSignals {
    /// Listen for SIGINT, SIGTERM and SIGHUP from now on.
    pub(crate) fn listen() -> Result<Self, ControlError> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Ok(Self {
                interrupt: signal(SignalKind::interrupt())?,
                terminate: signal(SignalKind::terminate())?,
                hangup: signal(SignalKind::hangup())?,
            })
        }
        #[cfg(not(unix))]
        Ok(Self {})
    }

    /// The next SIGINT or SIGTERM, or SIGHUP too when `hang_up`: a closed terminal stops the process as SIGTERM does, but a repeated hang-up never forces a shutdown already under way.
    pub(crate) async fn next(&mut self, hang_up: bool) -> Result<(), ControlError> {
        #[cfg(unix)]
        tokio::select! {
            _ = self.interrupt.recv() => {},
            _ = self.terminate.recv() => {},
            _ = self.hangup.recv(), if hang_up => {},
        }
        #[cfg(not(unix))]
        {
            let _ = hang_up;
            tokio::signal::ctrl_c().await?;
        }
        Ok(())
    }
}

/// The first SIGINT, SIGTERM or SIGHUP.
pub(crate) async fn shutdown_signal() -> Result<(), ControlError> {
    ShutdownSignals::listen()?.next(true).await
}

/// Stop sibling roles on a signal or role failure, then drain their shutdown results; a second SIGINT or SIGTERM meanwhile kills the workers' process groups at once instead of waiting for them.
pub(crate) async fn supervise_control_roles(
    mut tasks: JoinSet<Result<(), ControlError>>,
    stop: watch::Sender<bool>,
) -> Result<(), ControlError> {
    let mut signals: ShutdownSignals = ShutdownSignals::listen()?;
    let mut result = tokio::select! {
        result = tasks.join_next() => match result {
            Some(result) => result.map_err(ControlError::from).and_then(std::convert::identity),
            None => Ok(()),
        },
        result = signals.next(true) => result,
    };
    stop.send_replace(true);
    let mut forced: bool = false;
    loop {
        tokio::select! {
            joined = tasks.join_next() => {
                let Some(joined) = joined else {
                    break;
                };
                let joined = joined
                    .map_err(ControlError::from)
                    .and_then(std::convert::identity);
                result = result.and(joined);
            }
            signalled = signals.next(false), if !forced => {
                signalled?;
                forced = true;
                drop(write_output(&[(Stream::Stderr, FORCED_WORKER_STOP)]));
                stop_switch().kill_all();
            }
        }
    }
    result
}
