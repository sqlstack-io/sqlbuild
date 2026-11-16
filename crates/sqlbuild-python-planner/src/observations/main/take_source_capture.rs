//! The captured source columns, disarming the capture.

use crate::observations::_helpers::source_capture::CAPTURED;
use crate::observations::_helpers::source_capture::DescribedSources;

/// The captured source columns, disarming the capture.
pub fn take_source_capture() -> DescribedSources {
    CAPTURED
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_default()
}
