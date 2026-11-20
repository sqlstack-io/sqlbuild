//! Table rows for the `sqb dev` screen.

use crate::dev_screen::models::{DevScreen, ScreenAction, ScreenMessage};

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
    /// What the screen leaves when the session ends, or nothing while it runs.
    pub(super) close: fn(DevScreen) -> String,
    pub(super) expected_viewport: Vec<&'static str>,
}

/// Every session's layout at one terminal size, and what is wrong with it.
pub(super) struct LayoutTestCase {
    pub(super) description: &'static str,
    pub(super) size: (usize, usize),
    pub(super) expected_problems: Vec<String>,
}

/// A terminal `height` rows tall that keeps its scrollback: lines, `ESC[nF` (up, never above the top of the screen) and `ESC[J`.
pub(super) struct Terminal {
    lines: Vec<String>,
    cursor: usize,
    height: usize,
}

impl Terminal {
    pub(super) fn new(height: usize) -> Self {
        Self {
            lines: vec![String::new()],
            cursor: 0,
            height,
        }
    }

    pub(super) fn write(&mut self, text: &str) {
        let mut characters = text.chars().peekable();
        while let Some(character) = characters.next() {
            match (character, characters.peek()) {
                ('\u{1b}', Some('[')) => {
                    characters.next();
                    let mut argument: String = String::new();
                    let command: Option<char> = characters.by_ref().find(|next| {
                        let end: bool = next.is_ascii_alphabetic();
                        argument.extend((!end).then_some(*next));
                        end
                    });
                    self.escape(command, &argument);
                }
                ('\n', _) => {
                    self.cursor += 1;
                    let rows: usize = self.lines.len().max(self.cursor + 1);
                    self.lines.resize(rows, String::new());
                }
                (character, _) => self.lines[self.cursor].push(character),
            }
        }
    }

    /// `ESC[nF` moves up `n` rows, never above the top of the screen, to an emptied row; `ESC[J` clears below.
    fn escape(&mut self, command: Option<char>, argument: &str) {
        match command {
            Some('F') => {
                let top: usize = self.lines.len().saturating_sub(self.height);
                let up: usize = argument.parse().unwrap_or(1);
                self.cursor = self.cursor.saturating_sub(up).max(top);
                self.lines[self.cursor].clear();
            }
            Some('J') => {
                self.lines.truncate(self.cursor + 1);
                self.lines[self.cursor].clear();
            }
            _ => {}
        }
    }

    /// Every line written, scrollback included, without the cursor's empty last line.
    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.lines.clone();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    }

    /// What the screen shows: the last `height` rows, the cursor's row included.
    pub(super) fn viewport(&self) -> Vec<String> {
        let skip: usize = self.lines.len().saturating_sub(self.height);
        let mut rows: Vec<String> = self.lines[skip..].to_vec();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows
    }
}

/// Keys after ready: the actions they ask for.
pub(super) struct KeyTestCase {
    pub(super) description: &'static str,
    pub(super) keys: &'static str,
    pub(super) expected_actions: Vec<ScreenAction>,
}
