//! Whether Ctrl+C reached the current command, once the watch has read every signal delivered so far.

use pyo3::prelude::Python;

use crate::native_host::_helpers::interrupt;
use crate::native_host::_helpers::watch_session::catch_up;

/// Whether the default handler or the watch observed Ctrl+C during the current command, once the watch has caught up; a watch that can't be asked is reported as unraisable, and what it has seen so far decides.
pub fn interrupted(py: Python<'_>) -> bool {
    if let Err(error) = catch_up(py) {
        error.write_unraisable(py, None);
    }
    interrupt::interrupt_observed()
}
