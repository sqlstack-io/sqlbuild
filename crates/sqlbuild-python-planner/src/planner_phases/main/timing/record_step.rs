//! Record a planning step timed by its caller.

use crate::planner_phases::_helpers::timing::timings;

/// Add `seconds` to `step` for the plan command collecting on this thread, if one is.
#[inline]
pub fn record_step(step: &'static str, seconds: f64) {
    timings::record(step, seconds);
}
