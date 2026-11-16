//! Public constants of diagnostics.

use crate::diagnostics::_helpers::getters::read_build_phase_timing_tracker;
use crate::diagnostics::models::BuildPhaseTimingTracker;
use crate::records::main::registry::class_of::class_of;
use crate::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<BuildPhaseTimingTracker>,
    read: read_build_phase_timing_tracker,
}];
