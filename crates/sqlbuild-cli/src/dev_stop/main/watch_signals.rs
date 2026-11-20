//! Turn SIGINT and SIGTERM into stop requests.

use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::models::StopReason;

/// Every SIGINT or SIGTERM asks the switch to stop; the second forces it.
pub(crate) fn watch_signals() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::spawn(async move {
            loop {
                let reason: StopReason = tokio::select! {
                    received = interrupt.recv() => match received {
                        Some(()) => StopReason::Interrupt,
                        None => return,
                    },
                    received = terminate.recv() => match received {
                        Some(()) => StopReason::Terminate,
                        None => return,
                    },
                };
                stop_switch().request(reason);
            }
        });
    }
    #[cfg(not(unix))]
    tokio::spawn(async {
        while tokio::signal::ctrl_c().await.is_ok() {
            stop_switch().request(StopReason::Interrupt);
        }
    });
    Ok(())
}
