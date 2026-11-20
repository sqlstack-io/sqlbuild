//! The screen task: apply each message, write its text, and leave the result line behind.

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use sqlbuild_core::screen_text::main::redraw_rows::redraw_rows;
use sqlbuild_core::screen_text::main::render_log_line::render_log_line;
use sqlbuild_core::screen_text::main::render_result::render_result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

use crate::dev_screen::_helpers::apply_screen_message::apply_screen_message;
use crate::dev_screen::_helpers::capture::release_stderr;
use crate::dev_screen::_helpers::keys::KeyReader;
use crate::dev_screen::_helpers::lines::{compose, physical_rows};
use crate::dev_screen::constants::{FAILURE_LOG_LINES, TICK};
use crate::dev_screen::main::terminal_size::terminal_size;
use crate::dev_screen::models::{DevScreen, ScreenAction, ScreenEnd, ScreenMessage};
use crate::dev_stop::main::stop_switch::stop_switch;

/// Apply each message and write what it draws until the session finishes; then leave its result.
pub(crate) async fn draw_screen(
    mut screen: DevScreen,
    (mut inbox, actions): (
        UnboundedReceiver<ScreenMessage>,
        UnboundedSender<ScreenAction>,
    ),
    mut finished: oneshot::Receiver<ScreenEnd>,
    (keys, terminal): (Option<KeyReader>, Option<File>),
) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let end: Option<ScreenEnd> = loop {
        let message: ScreenMessage = tokio::select! {
            biased;
            message = inbox.recv() => match message {
                Some(message) => message,
                None => break None,
            },
            end = &mut finished => break end.ok(),
            _ = tick.tick(), if screen.live => {
                if let Some((columns, rows)) = terminal_size(terminal.as_ref()) {
                    screen.facts.columns = columns;
                    screen.facts.rows = rows;
                }
                ScreenMessage::Tick
            },
        };
        screen = apply(screen, message, (terminal.as_ref(), &actions));
    };
    while let Ok(message) = inbox.try_recv() {
        screen = apply(screen, message, (terminal.as_ref(), &actions));
    }
    drop(keys);
    let text: String = closing_text(screen, end.as_ref());
    write_screen(terminal.as_ref(), &text);
    if let Some(terminal) = terminal.as_ref() {
        release_stderr(terminal);
    }
    stop_switch().save_output(None);
}

fn apply(
    screen: DevScreen,
    message: ScreenMessage,
    (terminal, actions): (Option<&File>, &UnboundedSender<ScreenAction>),
) -> DevScreen {
    let clock: String = chrono::Local::now().format("%H:%M:%S").to_string();
    let (screen, output) = apply_screen_message(screen, message, Instant::now(), &clock);
    write_screen(terminal, &output.text);
    if output.action != ScreenAction::None {
        let _ = actions.send(output.action);
    }
    screen
}

/// The live rows cleared, the newest log lines when the screen did not explain the failure, then the result line.
pub(crate) fn closing_text(screen: DevScreen, end: Option<&ScreenEnd>) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(end) = end {
        if end.show_log && !screen.show_log {
            let skip: usize = screen.log.len().saturating_sub(FAILURE_LOG_LINES);
            for line in screen.log.iter().skip(skip) {
                lines.extend(render_log_line(line, &screen.facts));
            }
        }
        lines.extend(render_result(&end.result, &screen.facts));
    }
    let header_printed: bool = screen.header_printed;
    let live: bool = screen.live;
    let previous: usize = physical_rows(&screen.drawn, screen.facts.columns);
    if !live || !header_printed {
        let (_, text) = compose(
            DevScreen {
                live: false,
                ..screen
            },
            lines,
            Instant::now(),
        );
        return if live && previous > 0 {
            format!("{}{text}", redraw_rows(previous, &[]))
        } else {
            text
        };
    }
    redraw_rows(previous, &lines)
}

/// Write to the terminal stderr was taken from, or to stderr when it was not redirected.
fn write_screen(terminal: Option<&File>, text: &str) {
    if text.is_empty() {
        return;
    }
    match terminal {
        Some(mut terminal) => drop(terminal.write_all(text.as_bytes())),
        None => {
            let mut stderr = std::io::stderr().lock();
            drop(
                stderr
                    .write_all(text.as_bytes())
                    .and_then(|()| stderr.flush()),
            );
        }
    }
}
