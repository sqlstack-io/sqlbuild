//! A worker interpreter retains the native claim owner; it never opens the control database.

use pyo3::exceptions::PyRuntimeError;
use sqlbuild_orchestration::worker::models::{WorkerClient, WorkerSession};

#[derive(Debug)]
pub(crate) struct WorkerOwner {
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) client: WorkerClient,
    pub(crate) session: WorkerSession,
}

pub(crate) fn unavailable() -> pyo3::PyErr {
    PyRuntimeError::new_err("native worker session is closed or unavailable")
}

impl WorkerOwner {
    pub(crate) fn close(self) {
        drop(self.session);
        drop(self.client);
        self.runtime
            .shutdown_timeout(std::time::Duration::from_secs(1));
    }
}
