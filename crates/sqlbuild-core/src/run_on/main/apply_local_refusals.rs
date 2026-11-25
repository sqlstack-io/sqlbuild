//! Local tests that failed where the warehouse would not: why, and in `auto` where they then ran.

use crate::run_on::constants::WAREHOUSE;
use crate::run_on::models::{LocalRefusal, RunOn, RunOnDecision};

/// Each named test's refusal from its local run; in `auto` the test ran again on the warehouse.
pub fn apply_local_refusals(
    decisions: &mut [RunOnDecision],
    refused: Vec<(String, LocalRefusal)>,
    mode: RunOn,
) {
    for (name, refusal) in refused {
        if let Some(decision) = decisions.iter_mut().find(|decision| decision.name == name) {
            if mode == RunOn::Auto {
                WAREHOUSE.clone_into(&mut decision.run_on);
            }
            decision.refusals = vec![refusal];
        }
    }
}
