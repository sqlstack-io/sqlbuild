//! Apply one message to the `sqb dev` screen.

use std::time::Instant;

use sqlbuild_core::screen_text::models::PhaseState;

use crate::dev_screen::_helpers::lines::{compose, ready_lines};
use crate::dev_screen::_helpers::transitions::{
    child_event, client_text, key_pressed, logged, run_reported, step_finished, step_started,
    warehouse_settled,
};
use crate::dev_screen::models::{DevScreen, ScreenAction, ScreenMessage, ScreenOutput};

/// Change `screen` by `message` at `now` (local wall time `clock`), returning the text to write.
pub(crate) fn apply_screen_message(
    screen: &mut DevScreen,
    message: ScreenMessage,
    now: Instant,
    clock: &str,
) -> ScreenOutput {
    let mut action: ScreenAction = ScreenAction::None;
    let mut raw: String = String::new();
    let mut lines: Vec<String> = match message {
        ScreenMessage::Started { step, detail } => step_started(screen, (step, detail), now),
        ScreenMessage::Finished { step, detail } => {
            step_finished(screen, (step, PhaseState::Ok), detail, now)
        }
        ScreenMessage::Child(event) => child_event(screen, event, now, clock),
        ScreenMessage::Warehouse { adapter, state } => {
            let (closing, lines) = warehouse_settled(screen, (adapter, state), now, clock);
            raw = closing;
            lines
        }
        ScreenMessage::ClientOutput(text) => {
            return ScreenOutput {
                text: client_text(screen, &text, now),
                action,
            };
        }
        ScreenMessage::Run {
            name,
            detail,
            finished,
        } => run_reported(screen, (name, detail), finished, clock),
        ScreenMessage::Log(line) => logged(screen, line),
        ScreenMessage::Key(key) => {
            let (shown, asked) = key_pressed(screen, key);
            action = asked;
            shown
        }
        ScreenMessage::Tick => {
            if !screen.live || screen.ready || screen.client_output {
                return ScreenOutput {
                    text: String::new(),
                    action,
                };
            }
            screen.frame += 1;
            Vec::new()
        }
    };
    if screen.event_gap && !lines.is_empty() {
        screen.event_gap = false;
        lines.insert(0, String::new());
    }
    lines.extend(ready_lines(screen, now));
    ScreenOutput {
        text: compose(screen, &raw, &lines, now),
        action,
    }
}
