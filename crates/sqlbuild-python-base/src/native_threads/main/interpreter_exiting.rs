//! Whether an exit hook has started stopping native daemon threads.

use crate::native_threads::_helpers::exit_signal;

/// Whether an exit hook has started stopping native daemon threads.
#[inline]
pub fn interpreter_exiting() -> bool {
    exit_signal::interpreter_exiting()
}
