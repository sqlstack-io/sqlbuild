//! Whether the default handler observed Ctrl+C during the current command.

use crate::native_host::_helpers::interrupt;

/// Whether the default handler observed Ctrl+C during the current command.
#[inline]
pub fn interrupted() -> bool {
    interrupt::interrupt_observed()
}
