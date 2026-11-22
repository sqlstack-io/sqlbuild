//! Apply one message to the `sqb dev` screen.

use std::time::Instant;

use sqlbuild_core::screen_text::models::PhaseState;

use crate::dev_screen::_helpers::exposure::exposed;
use crate::dev_screen::_helpers::lines::{compose, ready_lines};
use crate::dev_screen::_helpers::transitions::{
    child_event, client_text, key_pressed, logged, run_cancelling, run_reported, service_stopped,
    step_failed, step_finished, step_started, warehouse_settled,
};
use crate::dev_screen::models::{DevScreen, ScreenAction, ScreenMessage, ScreenOutput};

/// `screen` changed by `message` at `now` (local wall time `clock`), and the text to write.
pub(crate) fn apply_screen_message(
    screen: DevScreen,
    message: ScreenMessage,
    now: Instant,
    clock: &str,
) -> (DevScreen, ScreenOutput) {
    let mut action: ScreenAction = ScreenAction::None;
    let (screen, lines): (DevScreen, Vec<String>) = match message {
        ScreenMessage::Started { step, detail } => step_started(screen, (step, detail), now),
        ScreenMessage::Finished { step, detail } => {
            step_finished(screen, (step, PhaseState::Ok), detail, now)
        }
        ScreenMessage::Failed {
            step,
            detail,
            problem,
        } => step_failed(screen, (step, detail, problem), now),
        ScreenMessage::Child(event) => child_event(screen, event, now, clock),
        ScreenMessage::Location(location) => (DevScreen { location, ..screen }, Vec::new()),
        ScreenMessage::Exposed(who) => exposed(screen, who),
        ScreenMessage::Triggers(triggers) => (DevScreen { triggers, ..screen }, Vec::new()),
        ScreenMessage::Warehouse { adapter, state } => {
            warehouse_settled(screen, (adapter, state), now, clock)
        }
        ScreenMessage::ClientOutput(text) => {
            if !screen.live {
                let (mut screen, mut output) = compose(screen, Vec::new(), now);
                if !text.is_empty() {
                    screen.client_partial = !text.ends_with('\n');
                }
                output.push_str(&text);
                return (
                    screen,
                    ScreenOutput {
                        text: output,
                        action,
                    },
                );
            }
            (client_text(screen, &text, now), Vec::new())
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
            let frame: usize = screen.frame + 1;
            (DevScreen { frame, ..screen }, Vec::new())
        }
        ScreenMessage::Stopping => {
            let stopping = screen.stopping.or(Some(now));
            (
                DevScreen {
                    stopping,
                    prompt: None,
                    ..screen
                },
                Vec::new(),
            )
        }
        ScreenMessage::Cancelling { label } => {
            let lines: Vec<String> = run_cancelling(&screen, label);
            (screen, lines)
        }
        ScreenMessage::Stopped { label, elapsed } => {
            let lines: Vec<String> = service_stopped(&screen, label, elapsed);
            (screen, lines)
        }
    };
    let (screen, mut lines) = (screen, lines);
    let (screen, ready) = if screen.stopping.is_some() {
        (screen, Vec::new())
    } else {
        ready_lines(screen)
    };
    lines.extend(ready);
    let (screen, text) = compose(screen, lines, now);
    (screen, ScreenOutput { text, action })
}
