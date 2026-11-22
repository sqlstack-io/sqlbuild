//! Turn SIGINT, SIGTERM and SIGHUP into stop requests.

use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::models::StopReason;

/// Every SIGINT, SIGTERM or SIGHUP asks the switch to stop; the second forces it.
pub(crate) fn watch_signals() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        let mut hangup = signal(SignalKind::hangup())?;
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
                    received = hangup.recv() => match received {
                        Some(()) => StopReason::Hangup,
                        None => return,
                    },
                };
                stop_switch().request(reason);
            }
        });
    }
    #[cfg(not(unix))]
    tokio::spawn(async {
        loop {
            match tokio::signal::ctrl_c().await {
                Ok(()) => stop_switch().request(StopReason::Interrupt),
                Err(_) => return,
            }
        }
    });
    Ok(())
}
