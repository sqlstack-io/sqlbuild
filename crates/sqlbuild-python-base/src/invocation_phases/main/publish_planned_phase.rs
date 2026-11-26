//! `publish_planned_phase`: the held planned phase, with what the plan will run.

use pyo3::prelude::{PyResult, Python};

use crate::invocation_phases::_helpers::held::HELD_PLANNED;
use crate::invocation_phases::constants::PLANNED_PHASE;
use crate::invocation_phases::models::{HeldPhase, InvocationPhase};
use crate::native_host::main::publish_invocation_phase::publish_invocation_phase;

/// Publish the planned phase this thread holds, named by `summary` such as `3 selected · 1 Python node`.
pub fn publish_planned_phase(py: Python<'_>, (summary, count): (String, usize)) -> PyResult<()> {
    let held: HeldPhase = HELD_PLANNED
        .with(|held| held.borrow_mut().take())
        .unwrap_or_default();
    publish_invocation_phase(
        py,
        &InvocationPhase {
            phase: PLANNED_PHASE,
            duration_ms: held.duration_ms,
            count: Some(i64::try_from(count).unwrap_or(i64::MAX)),
            name: Some(summary),
            detail: held.detail,
        },
    )
}
