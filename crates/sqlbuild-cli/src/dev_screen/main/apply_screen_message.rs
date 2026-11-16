//! Apply one message to the `sqb dev` screen.

use std::time::Instant;

use sqlbuild_core::screen_text::models::PhaseState;

use crate::dev_screen::_helpers::lines::{compose, ready_lines};
use crate::dev_screen::_helpers::transitions::{
    child_event, client_text, key_pressed, located, logged, run_reported, step_finished,
    step_started, warehouse_settled,
};
use crate::dev_screen::models::{DevScreen, DevStep, ScreenAction, ScreenMessage, ScreenOutput};

/// `screen` changed by `message` at `now` (local wall time `clock`), and the text to write.
pub(crate) fn apply_screen_message(
    screen: DevScreen,
    message: ScreenMessage,
    now: Instant,
    clock: &str,
) -> (DevScreen, ScreenOutput) {
    let mut action: ScreenAction = ScreenAction::None;
    let mut raw: String = String::new();
    let (mut screen, mut lines): (DevScreen, Vec<String>) = match message {
        ScreenMessage::Started { step, detail } => step_started(screen, (step, detail), now),
        ScreenMessage::Finished { step, detail } => {
            step_finished(screen, (step, PhaseState::Ok), detail, now)
        }
        ScreenMessage::Child(event) => child_event(screen, event, now, clock),
        ScreenMessage::Location(location) => located(screen, location),
        ScreenMessage::Triggers(triggers) => (DevScreen { triggers, ..screen }, Vec::new()),
        ScreenMessage::Warehouse { adapter, state } => {
            let (screen, closing, lines) = warehouse_settled(screen, (adapter, state), now, clock);
            raw = closing;
            (screen, lines)
        }
        ScreenMessage::ClientOutput(text) => {
            let (screen, text) = client_text(screen, &text, now);
            return (screen, ScreenOutput { text, action });
        }
        ScreenMessage::Run {
            name,
            detail,
            finished,
        } => {
            let lines: Vec<String> = run_reported(&screen, (name, detail), finished, clock);
            (screen, lines)
        }
        ScreenMessage::Log(line) => logged(screen, line),
        ScreenMessage::Key(key) => {
            let (screen, shown, asked) = key_pressed(screen, key);
            action = asked;
            (screen, shown)
        }
        ScreenMessage::Tick => {
            if !screen.live || screen.ready || screen.client_output || connecting(&screen) {
                return (
                    screen,
                    ScreenOutput {
                        text: String::new(),
                        action,
                    },
                );
            }
            let frame: usize = screen.frame + 1;
            (DevScreen { frame, ..screen }, Vec::new())
        }
    };
    if screen.event_gap && !lines.is_empty() {
        screen.event_gap = false;
        lines.insert(0, String::new());
    }
    let (screen, ready) = ready_lines(screen, now);
    lines.extend(ready);
    let (screen, text) = compose(screen, (&raw, &lines), now);
    (screen, ScreenOutput { text, action })
}

/// The foreground connection may prompt on the terminal itself, so nothing redraws over it.
fn connecting(screen: &DevScreen) -> bool {
    screen.steps.iter().any(|row| {
        row.step == DevStep::Warehouse && matches!(row.state, PhaseState::Running { .. })
    })
}
