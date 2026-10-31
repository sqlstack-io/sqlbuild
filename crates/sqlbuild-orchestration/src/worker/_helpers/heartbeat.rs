//! A dedicated native runtime keeps heartbeat IO independent of warehouse work and Python's GIL.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::runs::models::RunExecution;
use crate::worker::errors::WorkerError;
use crate::worker::models::{WorkerClient, WorkerSession};

async fn heartbeat_loop(
    client: WorkerClient,
    cancelled: Arc<AtomicBool>,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            _ = stop.changed() => return,
            _ = interval.tick() => {}
        }
        let result = tokio::select! {
            biased;
            _ = stop.changed() => return,
            result = client.heartbeat() => result,
        };
        match result {
            Ok(true) => cancelled.store(true, Ordering::Release),
            Err(WorkerError::Rejected(status))
                if matches!(status.as_u16(), 401 | 403 | 404 | 410) =>
            {
                cancelled.store(true, Ordering::Release);
                return;
            }
            Ok(false) | Err(_) => {}
        }
    }
}

pub(crate) async fn start_session(
    client: WorkerClient,
    execution: RunExecution,
) -> Result<WorkerSession, WorkerError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    let (stop, stopping) = tokio::sync::watch::channel(false);
    let (ready, started) = tokio::sync::oneshot::channel();
    let thread = std::thread::Builder::new()
        .name("sqb-worker-heartbeat".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready.send(Err(WorkerError::from(error)));
                    return;
                }
            };
            runtime.block_on(async move {
                let heartbeat = match client.connection.independent() {
                    Ok(connection) => WorkerClient {
                        connection,
                        ..client
                    },
                    Err(error) => {
                        let _ = ready.send(Err(WorkerError::from(error)));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                heartbeat_loop(heartbeat, signal, stopping).await;
            });
            runtime.shutdown_timeout(Duration::from_secs(1));
        })?;
    let session = WorkerSession {
        execution,
        cancel_requested: cancelled,
        stop,
        thread: Some(thread),
    };
    started
        .await
        .map_err(|_| WorkerError::Configuration("heartbeat thread stopped during startup"))??;
    Ok(session)
}
