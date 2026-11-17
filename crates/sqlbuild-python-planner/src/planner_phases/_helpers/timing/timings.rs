//! The planning steps recorded on this thread for one plan command.

use crate::planner_phases::constants::OTHER;
use crate::planner_phases::models::PlanTimings;
use std::cell::RefCell;

thread_local! {
    pub(crate) static STEPS: RefCell<Option<Vec<(&'static str, f64)>>> = const { RefCell::new(None) };
}

/// `recorded` steps within `total_seconds`, with `Other` added as what they leave uncovered.
pub(crate) fn timings_of(
    mut recorded: Vec<(&'static str, f64)>,
    total_seconds: f64,
) -> PlanTimings {
    let covered: f64 = recorded.iter().map(|(_, seconds)| seconds).sum();
    recorded.push((OTHER, (total_seconds - covered).max(0.0)));
    PlanTimings {
        total_seconds,
        steps: recorded,
    }
}

/// Add `seconds` to `step` for the plan command collecting on this thread, if one is.
pub(crate) fn record(step: &'static str, seconds: f64) {
    STEPS.with(|steps| {
        if let Some(steps) = steps.borrow_mut().as_mut() {
            match steps.iter_mut().find(|(name, _)| *name == step) {
                Some((_, total)) => *total += seconds,
                None => steps.push((step, seconds)),
            }
        }
    });
}
