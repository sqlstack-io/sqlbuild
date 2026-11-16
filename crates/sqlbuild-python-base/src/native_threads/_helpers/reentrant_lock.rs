//! A native reentrant lock whose contended waits release the GIL and honor Python signals.

use crate::native_threads::models::ReentrantLock;
use std::sync::PoisonError;
use std::thread::ThreadId;
use std::time::Duration;

pub(crate) const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(5);
pub(crate) const POISONED: &str = "native reentrant lock is poisoned";

#[derive(Debug, Default)]
pub(crate) struct LockState {
    pub(crate) owner: Option<ThreadId>,
    pub(crate) depth: usize,
}

/// Release one acquisition even when the protected callback raises or panics.
pub(crate) struct LockGuard<'a>(pub(crate) &'a ReentrantLock);

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
