//! The screen task: apply each message, write its text, and leave the closing text behind.

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use sqlbuild_core::screen_text::main::redraw_rows::redraw_rows;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

use crate::dev_screen::_helpers::apply_screen_message::apply_screen_message;
use crate::dev_screen::_helpers::capture::release_stderr;
use crate::dev_screen::_helpers::keys::KeyReader;
use crate::dev_screen::constants::{FAILURE_LOG_LINES, TICK};
use crate::dev_screen::models::{DevScreen, ScreenAction, ScreenMessage};

/// Apply each message and write what it draws, until the session finishes; then leave the closing text.
pub(crate) async fn draw_screen(
    mut screen: DevScreen,
    (mut inbox, actions): (
        UnboundedReceiver<ScreenMessage>,
        UnboundedSender<ScreenAction>,
    ),
    mut finished: oneshot::Receiver<bool>,
    (keys, terminal): (Option<KeyReader>, Option<File>),
) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let failed: bool = loop {
        let message: ScreenMessage = tokio::select! {
            failed = &mut finished => break failed.unwrap_or(false),
            message = inbox.recv() => match message {
                Some(message) => message,
                None => break false,
            },
            _ = tick.tick(), if screen.live => ScreenMessage::Tick,
        };
        let clock: String = chrono::Local::now().format("%H:%M:%S").to_string();
        let output;
        (screen, output) = apply_screen_message(screen, message, Instant::now(), &clock);
        write_screen(terminal.as_ref(), &output.text);
        if output.action != ScreenAction::None {
            let _ = actions.send(output.action);
        }
    };
    drop(keys);
    if let Some(terminal) = terminal.as_ref() {
        release_stderr(terminal);
    }
    write_screen(None, &closing_text(&screen, failed));
}

/// What stays on the terminal after the screen stops: the live rows cleared and, on failure, the newest log lines that explain it.
fn closing_text(screen: &DevScreen, failed: bool) -> String {
    let mut text: String = if screen.live && screen.drawn > 0 {
        redraw_rows(screen.drawn, &[])
    } else {
        String::new()
    };
    if failed && !screen.show_log {
        let skip: usize = screen.log.len().saturating_sub(FAILURE_LOG_LINES);
        for line in screen.log.iter().skip(skip) {
            text.push_str(line);
            text.push('\n');
        }
    }
    text
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
