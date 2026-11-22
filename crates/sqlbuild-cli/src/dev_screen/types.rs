//! The parts of a running screen that other roles hold.

use std::sync::Arc;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::dev_screen::models::{KeyInput, ScreenAction, ScreenMessage};

/// Where to send messages, the actions keys ask for, and the key reader's input.
pub(crate) type DevScreenParts = (
    UnboundedSender<ScreenMessage>,
    UnboundedReceiver<ScreenAction>,
    Option<Arc<KeyInput>>,
);
