//! The execution owner drains its heartbeat before worker runtime teardown.

use std::sync::atomic::Ordering;

use crate::worker::models::WorkerSession;

impl WorkerSession {
    /// Cancellation and credential revocation are monotonic for one pinned worker invocation.
    #[must_use]
    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::Acquire)
    }
}

impl Drop for WorkerSession {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
