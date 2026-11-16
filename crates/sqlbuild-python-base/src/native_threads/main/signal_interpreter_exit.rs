//! Tell every native wait to stop waiting on Python work that may outlive the interpreter.

use crate::native_threads::_helpers::exit_signal;

/// Tell every native wait to stop waiting on Python work that may outlive the interpreter.
#[inline]
pub fn signal_interpreter_exit() {
    exit_signal::signal_interpreter_exit()
}
