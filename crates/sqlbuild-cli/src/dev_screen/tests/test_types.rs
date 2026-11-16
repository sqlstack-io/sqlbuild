//! Table rows for the `sqb dev` screen.

use crate::dev_screen::models::{ScreenAction, ScreenMessage};

/// What a terminal shows after a session, and the action each message asked for.
pub(super) struct Transcript {
    pub(super) rows: Vec<String>,
    pub(super) actions: Vec<ScreenAction>,
}

/// A session at `columns` (`None` for plain output) and every row it leaves on the terminal.
pub(super) struct ScreenTestCase {
    pub(super) description: &'static str,
    pub(super) columns: Option<usize>,
    pub(super) messages: Vec<ScreenMessage>,
    pub(super) expected_rows: Vec<String>,
}

/// A session ending in keys: the actions the keys ask for and the log lines left on screen.
pub(super) struct KeyTestCase {
    pub(super) description: &'static str,
    pub(super) columns: Option<usize>,
    pub(super) messages: Vec<ScreenMessage>,
    pub(super) keys: &'static str,
    pub(super) expected_actions: Vec<ScreenAction>,
    pub(super) expected_log: Vec<String>,
}
