//! Run the `sqb dev` screen as the only writer to the terminal.

use std::fs::File;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::dev_screen::_helpers::capture::capture_stderr;
use crate::dev_screen::_helpers::drawing::draw_screen;
use crate::dev_screen::_helpers::keys::KeyReader;
use crate::dev_screen::_helpers::new_dev_screen::new_dev_screen;
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
    let terminal: Option<File> = match capture_stderr(messages.clone()) {
        Ok(terminal) => terminal,
        Err(error) => {
            drop(messages.send(ScreenMessage::Log(format!(
                "stderr stays on the terminal: {error}"
            ))));
            None
        }
    };
    let screen: DevScreen = new_dev_screen(setup, Instant::now());
    let task = tokio::spawn(draw_screen(
        screen,
        (inbox, actions),
        finished,
        (keys, terminal),
    ));
    DevScreenHandle {
        messages,
        actions: asked,
        keys: gate,
        finish,
        task,
    }
}
