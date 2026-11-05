//! A worker owns only an API credential, never a control-store connection.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use uuid::Uuid;

use crate::api::models::ControlClient;
use crate::runs::models::RunExecution;

#[derive(Clone, Debug)]
pub struct WorkerClient {
    pub(crate) connection: ControlClient,
    pub(crate) run_url: reqwest::Url,
    pub(crate) run_id: Uuid,
    pub(crate) claim_id: Uuid,
}

/// Keep this owner alive through execution and terminal reporting, then stop and join its heartbeat.
#[derive(Debug)]
pub struct WorkerSession {
    pub execution: RunExecution,
    pub(crate) cancel_requested: Arc<AtomicBool>,
    pub(crate) stop: tokio::sync::watch::Sender<bool>,
    pub(crate) thread: Option<std::thread::JoinHandle<()>>,
}
