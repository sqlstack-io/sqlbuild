//! Time one planning step.

use crate::planner_phases::_helpers::timing::timings::record;
use std::time::Instant;

/// Run `body`, recording its wall time as `step`.
pub fn timed<T>(step: &'static str, body: impl FnOnce() -> T) -> T {
    let started: Instant = Instant::now();
    let result: T = body();
    record(step, started.elapsed().as_secs_f64());
    result
}
