//! The rows of live terminal screens: phase checklists, tree groups and the `sqb dev` screen.

use std::time::Duration;

use crate::cli_text::models::CliStyle;

/// What the terminal supports and how wide each row may be.
#[derive(Clone, Copy, Debug)]
pub struct ScreenFacts {
    /// The cells a row may use; rows are cut to this width and never wrap.
    pub columns: usize,
    /// Colours, or none in plain mode.
    pub style: CliStyle,
    /// Whether to wrap locations in OSC 8 hyperlinks.
    pub links: bool,
    /// The spaces every row starts with.
    pub indent: usize,
}

/// How one span of a row is styled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Muted,
    Strong,
    Success,
    Warning,
    Error,
    Change,
    Link,
}

/// One styled span of a row, optionally a hyperlink to `link`.
#[derive(Clone, Debug)]
pub struct Segment {
    pub text: String,
    pub tone: Tone,
    pub link: Option<String>,
}

/// The state glyph a phase line starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseState {
    Pending,
    /// Running, drawn with spinner frame `frame`.
    Running {
        frame: usize,
    },
    Ok,
    Warn,
    Fail,
}

/// One line of a phase checklist.
#[derive(Clone, Debug)]
pub struct PhaseLine {
    pub state: PhaseState,
    pub label: String,
    pub detail: String,
    pub elapsed: Option<Duration>,
}

/// Where a phase line puts its time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseLayout {
    /// Label, detail, then the time right-aligned after the widest detail.
    TimeLast,
    /// Label, the time right-aligned beside it, then the detail.
    TimeFirst,
}

/// A titled group of entries drawn as a tree, bounded to `limit` entries.
#[derive(Clone, Debug)]
pub struct TreeGroup {
    pub title: String,
    /// Every entry the group has, shown or not.
    pub total: usize,
    pub entries: Vec<TreeEntry>,
    pub limit: usize,
    /// The flag that shows every entry, such as `--verbose`.
    pub flag: String,
}

/// One named entry of a tree group and its key/value rows.
#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub name: String,
    pub rows: Vec<TreeRow>,
}

/// One `key  value` row under a tree entry; `link` makes the value clickable.
#[derive(Clone, Debug)]
pub struct TreeRow {
    pub key: String,
    pub value: String,
    pub tone: Tone,
    pub link: Option<String>,
}

/// The first line of the `sqb dev` screen.
#[derive(Clone, Debug)]
pub struct DevHeader {
    pub version: String,
    pub project: String,
    pub target: String,
}

/// Whether the dev target's warehouse connection is open.
#[derive(Clone, Debug)]
pub enum WarehouseState {
    Connected {
        detail: String,
    },
    /// Not connected, and no background step will prompt for sign-in.
    NotConnected {
        reason: String,
    },
    Failed {
        message: String,
    },
}

/// The card `sqb dev` shows once the location is ready.
#[derive(Clone, Debug)]
pub struct ReadyCard {
    pub version: String,
    pub ready_in: Duration,
    pub ui_url: String,
    pub location: String,
    pub adapter: String,
    pub warehouse: WarehouseState,
    pub triggers: String,
    pub watching: String,
    /// The env file the session loaded and how to reload it, `None` without a project env file.
    pub env_file: Option<String>,
}

/// The glyph and colour of one event line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventMark {
    Changed,
    Succeeded,
    Failed,
    Restarted,
    Started,
    Warning,
}

/// One timestamped event line of the `sqb dev` screen, with any indented notes below it.
#[derive(Clone, Debug)]
pub struct EventLine {
    /// The local time, `HH:MM:SS`.
    pub clock: String,
    pub mark: EventMark,
    pub title: String,
    pub detail: String,
    pub elapsed: Option<Duration>,
    pub notes: Vec<Vec<Segment>>,
}

/// One key of the hotkey bar.
#[derive(Clone, Debug)]
pub struct Hotkey {
    pub key: char,
    pub label: String,
}
