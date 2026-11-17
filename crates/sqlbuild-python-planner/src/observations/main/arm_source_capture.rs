//! Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.

use crate::observations::_helpers::source_capture::CAPTURED;

/// Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.
pub fn arm_source_capture() {
    CAPTURED.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
}
