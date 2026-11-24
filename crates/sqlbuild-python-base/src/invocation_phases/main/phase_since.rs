//! `phase_since`: a phase that started at a given instant and has just finished.

use std::time::Instant;

use crate::invocation_phases::models::InvocationPhase;

/// `phase`, timed from `started` to now.
#[must_use]
pub fn phase_since(phase: &str, started: Instant) -> InvocationPhase<'_> {
    InvocationPhase {
        phase,
        duration_ms: Some(started.elapsed().as_secs_f64() * 1000.0),
        ..InvocationPhase::default()
    }
}
