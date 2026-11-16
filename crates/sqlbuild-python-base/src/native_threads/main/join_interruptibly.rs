//! Join `thread` without the GIL; on a signal call `interrupt`, finish the join and return the signal.

use crate::native_threads::_helpers::interruptible_join;
use crate::native_threads::models::Joined;
use pyo3::prelude::Python;
use std::thread::JoinHandle;

/// Join `thread` without the GIL; on a signal call `interrupt`, finish the join and return the signal.
#[inline]
pub fn join_interruptibly<T: Send>(
    py: Python<'_>,
    thread: JoinHandle<T>,
    interrupt: impl FnOnce(),
) -> Joined<T> {
    interruptible_join::join_interruptibly(py, thread, interrupt)
}
