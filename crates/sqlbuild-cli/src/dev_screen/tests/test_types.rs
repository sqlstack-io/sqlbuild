//! Table rows for the `sqb dev` screen.

use crate::dev_screen::models::{ScreenAction, ScreenEnd, ScreenMessage};

/// What a terminal shows after a session, everything it kept, and the action each message asked for.
pub(super) struct Transcript {
    pub(super) viewport: Vec<String>,
    pub(super) scrollback: Vec<String>,
    pub(super) actions: Vec<ScreenAction>,
}

/// A session at `size` (`None` for plain output) and the screen it leaves.
pub(super) struct ScreenTestCase {
    pub(super) description: &'static str,
    pub(super) size: Option<(usize, usize)>,
    pub(super) messages: Vec<ScreenMessage>,
    pub(super) end: Option<ScreenEnd>,
    pub(super) expected_viewport: Vec<&'static str>,
}

/// Keys after ready: the actions they ask for.
pub(super) struct KeyTestCase {
    pub(super) description: &'static str,
    pub(super) keys: &'static str,
    pub(super) expected_actions: Vec<ScreenAction>,
}
