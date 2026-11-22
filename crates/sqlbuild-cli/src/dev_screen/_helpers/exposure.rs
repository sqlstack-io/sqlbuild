//! The warning `sqb dev` prints for good when it is open beyond this machine.

use sqlbuild_core::screen_text::main::render_step_line::render_step_line;
use sqlbuild_core::screen_text::models::{PhaseState, StepLine};

use crate::dev_screen::constants::NO_SIGN_IN;
use crate::dev_screen::models::DevScreen;

/// Print who gets in without signing in, and keep it for the ready box and the log.
pub(crate) fn exposed(mut screen: DevScreen, who: String) -> (DevScreen, Vec<String>) {
    let line: StepLine = StepLine {
        state: PhaseState::Warn,
        label: NO_SIGN_IN.to_owned(),
        elapsed: None,
        detail: who.clone(),
    };
    let lines: Vec<String> = render_step_line(&line, &screen.facts, true);
    screen.log.push_back(format!("{NO_SIGN_IN}: {who}"));
    screen.exposure = who;
    (screen, lines)
}
