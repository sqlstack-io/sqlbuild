//! Run the `sqb dev` screen as the only writer to the terminal.

use std::io::Write;
use std::time::Instant;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

use crate::dev_screen::_helpers::keys::KeyReader;
use crate::dev_screen::constants::{FAILURE_LOG_LINES, TICK};
use crate::dev_screen::main::apply_screen_message::apply_screen_message;
use crate::dev_screen::main::new_dev_screen::new_dev_screen;
use crate::dev_screen::models::{
    DevScreen, DevScreenHandle, ScreenAction, ScreenMessage, ScreenSetup,
};

/// Draw `setup`'s screen from the messages sent to the handle; a live screen also reads hotkeys.
pub(crate) fn start_dev_screen(setup: ScreenSetup) -> DevScreenHandle {
    let live: bool = setup.live;
    let (messages, inbox) = mpsc::unbounded_channel::<ScreenMessage>();
    let (actions, asked) = mpsc::unbounded_channel::<ScreenAction>();
    let (finish, finished) = oneshot::channel::<bool>();
    let keys: Option<KeyReader> = if live {
        KeyReader::start(messages.clone())
    } else {
        None
    };
    let gate = keys.as_ref().map(KeyReader::gate);
    let screen: DevScreen = new_dev_screen(setup, Instant::now());
    let task = tokio::spawn(draw_screen(screen, (inbox, actions), finished, keys));
    DevScreenHandle {
        messages,
        actions: asked,
        keys: gate,
        finish,
        task,
    }
}

async fn draw_screen(
    mut screen: DevScreen,
    (mut inbox, actions): (UnboundedReceiver<ScreenMessage>, UnboundedSender<ScreenAction>),
    mut finished: oneshot::Receiver<bool>,
    keys: Option<KeyReader>,
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
        write_screen(&output.text);
        if output.action != ScreenAction::None {
            let _ = actions.send(output.action);
        }
    };
    drop(keys);
    write_screen(&closing_text(&screen, failed));
}

/// What stays on the terminal after the screen stops: on failure, the newest log lines that explain it.
fn closing_text(screen: &DevScreen, failed: bool) -> String {
    let mut text: String = String::new();
    if failed && !screen.show_log {
        let skip: usize = screen.log.len().saturating_sub(FAILURE_LOG_LINES);
        for line in screen.log.iter().skip(skip) {
            text.push_str(line);
            text.push('\n');
        }
    }
    text
}

fn write_screen(text: &str) {
    if text.is_empty() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    drop(stderr.write_all(text.as_bytes()).and_then(|()| stderr.flush()));
}
