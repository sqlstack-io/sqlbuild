//! The interpreter-exit signal native waits poll so Python daemon threads leave native frames before finalization.

use std::sync::atomic::{AtomicBool, Ordering};

static EXIT_SIGNAL: AtomicBool = AtomicBool::new(false);

/// Whether an exit hook has started stopping native daemon threads.
pub(crate) fn interpreter_exiting() -> bool {
    EXIT_SIGNAL.load(Ordering::SeqCst)
}

/// Tell every native wait to stop waiting on Python work that may outlive the interpreter.
pub(crate) fn signal_interpreter_exit() {
    EXIT_SIGNAL.store(true, Ordering::SeqCst);
}
