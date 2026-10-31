//! A worker interpreter retains the native claim owner; it never opens the control database.

use std::sync::Mutex;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{PyRef, PyResult, Python};
use pyo3::{pyclass, pymethods};
use sqlbuild_orchestration::runs::models::RunChange;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use sqlbuild_orchestration::worker::models::{WorkerClient, WorkerSession};
use uuid::Uuid;

use crate::bindings::_helpers::compiled_project::constants::REPORT_ATTEMPTS;
use crate::bindings::_helpers::compiled_project::snapshot::NativeOrchestrationSnapshot;

struct WorkerOwner {
    runtime: tokio::runtime::Runtime,
    client: WorkerClient,
    session: WorkerSession,
}

/// Closing this owner stops and joins heartbeat IO but does not claim warehouse work has stopped.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct NativeOrchestrationWorker {
    owner: Mutex<Option<WorkerOwner>>,
}

impl NativeOrchestrationWorker {
    /// SIGTERM may arrive before the periodic heartbeat observes the durable cancellation fact.
    pub(crate) fn refresh_cancellation(&self, py: Python<'_>) -> PyResult<bool> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            let owner = owner.as_ref().ok_or_else(unavailable)?;
            if owner.session.cancel_requested() {
                return Ok(true);
            }
            owner
                .runtime
                .block_on(owner.client.heartbeat())
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Native execution reads the retained request, never caller-provided selection or settings.
    pub(crate) fn execution(
        &self,
        py: Python<'_>,
    ) -> PyResult<sqlbuild_orchestration::runs::models::RunExecution> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            Ok(owner
                .as_ref()
                .ok_or_else(unavailable)?
                .session
                .execution
                .clone())
        })
    }

    /// A bounded retry keeps the exact event identity across an uncertain acknowledgement.
    pub(crate) fn change(&self, py: Python<'_>, key: &str, change: RunChange) -> PyResult<()> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            let owner = owner.as_ref().ok_or_else(unavailable)?;
            let event_id = Uuid::new_v5(&owner.session.execution.claim.claim_id, key.as_bytes());
            owner.runtime.block_on(async {
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    match owner.client.report(event_id, change.clone()).await {
                        Ok(_) => return Ok(()),
                        Err(sqlbuild_orchestration::worker::errors::WorkerError::Transport(_))
                            if attempts < REPORT_ATTEMPTS =>
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                        Err(error) => return Err(PyRuntimeError::new_err(error.to_string())),
                    }
                }
            })
        })
    }

    /// Verify this retained baseline before the caller applies run settings or opens a warehouse.
    pub(crate) fn start(
        snapshot: &NativeOrchestrationSnapshot,
        py: Python<'_>,
        (server_url, bearer): (&str, &str),
        (run_id, claim_id): (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        let identity = (
            Uuid::parse_str(&run_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
            Uuid::parse_str(&claim_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let connected = runtime.block_on(async {
                let client = connect_worker(server_url, bearer, identity)?;
                let session = start_worker(&client, &snapshot.input).await?;
                Ok::<_, sqlbuild_orchestration::worker::errors::WorkerError>((client, session))
            });
            match connected {
                Ok((client, session)) => Ok(NativeOrchestrationWorker {
                    owner: Mutex::new(Some(WorkerOwner {
                        runtime,
                        client,
                        session,
                    })),
                }),
                Err(error) => {
                    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
                    Err(PyRuntimeError::new_err(error.to_string()))
                }
            }
        })
    }
}

#[pymethods]
impl NativeOrchestrationWorker {
    /// The frozen request supplies selection and settings only after baseline verification.
    fn execution_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            let owner = owner.as_ref().ok_or_else(unavailable)?;
            serde_json::to_string(&owner.session.execution)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Cancellation remains sticky; a closed worker cannot authorize further work.
    pub(crate) fn cancel_requested(&self, py: Python<'_>) -> bool {
        py.detach(|| {
            self.owner.lock().map_or(true, |owner| {
                owner
                    .as_ref()
                    .is_none_or(|owner| owner.session.cancel_requested())
            })
        })
    }

    /// Retry an uncertain report with the identical event ID and change, never a new identity.
    fn report(&self, py: Python<'_>, event_id: &str, change_json: &str) -> PyResult<bool> {
        let event_id =
            Uuid::parse_str(event_id).map_err(|error| PyValueError::new_err(error.to_string()))?;
        let change: RunChange = serde_json::from_str(change_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            let owner = owner.as_ref().ok_or_else(unavailable)?;
            owner
                .runtime
                .block_on(owner.client.report(event_id, change))
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.owner.lock().map_err(|_| unavailable())?.is_none() {
            return Err(unavailable());
        }
        Ok(slf)
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _kind: &pyo3::Bound<'_, pyo3::PyAny>,
        _value: &pyo3::Bound<'_, pyo3::PyAny>,
        _traceback: &pyo3::Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }

    /// Join the heartbeat without publishing a terminal outcome or releasing physical claims.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let mut owner = self.owner.lock().map_err(|_| unavailable())?;
            if let Some(active) = owner.take() {
                active.close();
            }
            Ok(())
        })
    }
}

fn unavailable() -> pyo3::PyErr {
    PyRuntimeError::new_err("native worker session is closed or unavailable")
}

impl WorkerOwner {
    fn close(self) {
        drop(self.session);
        drop(self.client);
        self.runtime
            .shutdown_timeout(std::time::Duration::from_secs(1));
    }
}

impl Drop for NativeOrchestrationWorker {
    /// GC can hold the GIL; heartbeat teardown joins native IO and never needs Python.
    fn drop(&mut self) {
        if let Ok(owner) = self.owner.get_mut()
            && let Some(owner) = owner.take()
        {
            owner.close();
        }
    }
}
