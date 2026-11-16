//! Public records of native threads.

use crate::native_threads::_helpers::reentrant_lock::LockGuard;
use crate::native_threads::_helpers::reentrant_lock::LockState;
use crate::native_threads::_helpers::reentrant_lock::POISONED;
use crate::native_threads::_helpers::reentrant_lock::SIGNAL_POLL_INTERVAL;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::ThreadId;
use std::thread::current;

/// A joined thread's outcome, or the signal raised while waiting for it.
#[derive(Debug)]
pub enum Joined<T> {
    Finished(std::thread::Result<T>),
    Interrupted(PyErr),
}

/// Serialize native operations that may reenter through Python callbacks.
#[derive(Debug, Default)]
pub struct ReentrantLock {
    pub(crate) state: Mutex<LockState>,
    pub(crate) available: Condvar,
}

impl ReentrantLock {
    /// Run `body` under the lock, checking pending Python signals while another thread owns it.
    pub fn with_lock<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        while !py
            .detach(|| self.acquire_or_wait())
            .map_err(PyRuntimeError::new_err)?
        {
            py.check_signals()?;
        }
        let _guard = LockGuard(self);
        body()
    }

    pub(crate) fn acquire_or_wait(&self) -> Result<bool, &'static str> {
        let owner: ThreadId = current().id();
        let mut state = self.state.lock().map_err(|_| POISONED)?;
        if state.owner.is_none() || state.owner == Some(owner) {
            state.owner = Some(owner);
            state.depth += 1;
            return Ok(true);
        }
        let (_state, _) = self
            .available
            .wait_timeout(state, SIGNAL_POLL_INTERVAL)
            .map_err(|_| POISONED)?;
        Ok(false)
    }
}
