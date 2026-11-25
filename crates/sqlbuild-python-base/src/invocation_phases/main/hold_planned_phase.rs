//! `hold_planned_phase`: keep the planned phase back until the plan's Python nodes are known.

use crate::invocation_phases::_helpers::held::HELD_PLANNED;
use crate::invocation_phases::models::HeldPhase;

/// Hold `phase` on this thread for `publish_planned_phase`, replacing any held before.
pub fn hold_planned_phase(phase: HeldPhase) {
    HELD_PLANNED.with(|held| *held.borrow_mut() = Some(phase));
}
