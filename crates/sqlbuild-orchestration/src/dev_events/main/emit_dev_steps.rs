//! Send step events to a `sqb dev` parent.

use crate::dev_events::main::emit_dev_event::emit_dev_event;
use crate::dev_events::models::{DevEvent, DevPhase, DevStepState};

/// Tell a `sqb dev` parent that each phase reached its state, all with `detail`.
pub fn emit_dev_steps(
    steps: &[(DevPhase, DevStepState)],
    detail: Option<String>,
) -> std::io::Result<()> {
    for &(phase, state) in steps {
        let detail: Option<String> = detail.clone();
        emit_dev_event(&DevEvent::Step {
            phase,
            state,
            detail,
        })?;
    }
    Ok(())
}
