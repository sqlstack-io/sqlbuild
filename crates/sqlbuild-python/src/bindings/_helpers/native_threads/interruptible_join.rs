//! Joining a native thread from Python's main thread without hiding Ctrl+C.

use std::thread::JoinHandle;
use std::time::Duration;

use pyo3::prelude::{PyErr, Python};

use crate::bindings::_helpers::native_host::interrupt::check_signals;

/// How often a join checks for pending signals such as Ctrl+C.
const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// A joined thread's outcome, or the signal raised while waiting for it.
pub(crate) enum Joined<T> {
    Finished(std::thread::Result<T>),
    Interrupted(PyErr),
}

/// Join `thread` without the GIL; on a signal call `interrupt`, finish the join and return the signal.
pub(crate) fn join_interruptibly<T: Send>(
    py: Python<'_>,
    thread: JoinHandle<T>,
    interrupt: impl FnOnce(),
) -> Joined<T> {
    while !thread.is_finished() {
        if let Err(signal) = check_signals(py) {
            interrupt();
            let _ = py.detach(|| thread.join());
            return Joined::Interrupted(signal);
        }
        py.detach(|| std::thread::sleep(SIGNAL_POLL_INTERVAL));
    }
    Joined::Finished(py.detach(|| thread.join()))
}
