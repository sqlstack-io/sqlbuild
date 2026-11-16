//! The captured source columns, disarming the capture.

use crate::observations::_helpers::source_capture;
use crate::observations::_helpers::source_capture::DescribedSources;

/// The captured source columns, disarming the capture.
#[inline]
pub fn take_source_capture() -> DescribedSources {
    source_capture::take_source_capture()
}
