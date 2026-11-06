//! A native reentrant lock whose contended waits release the GIL and honor Python signals.

use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::{ThreadId, current};
use std::time::Duration;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{PyResult, Python};

const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(5);
const POISONED: &str = "native reentrant lock is poisoned";

#[derive(Default)]
struct LockState {
    owner: Option<ThreadId>,
    depth: usize,
}

/// Serialize native operations that may reenter through Python callbacks.
#[derive(Default)]
pub(crate) struct ReentrantLock {
    state: Mutex<LockState>,
    available: Condvar,
}

/// Release one acquisition even when the protected callback raises or panics.
struct LockGuard<'a>(&'a ReentrantLock);

impl ReentrantLock {
    /// Run `body` under the lock, checking pending Python signals while another thread owns it.
    pub(crate) fn with_lock<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        while !py
            .detach(|| self.acquire_or_wait())
            .map_err(PyRuntimeError::new_err)?
        {
            py.check_signals()?;
        }
        let _guard = LockGuard(self);
        body()
    }

    fn acquire_or_wait(&self) -> Result<bool, &'static str> {
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

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.depth -= 1;
        if state.depth == 0 {
            state.owner = None;
            self.0.available.notify_one();
        }
    }
}
