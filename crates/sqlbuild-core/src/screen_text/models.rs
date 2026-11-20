//! The rows of the `sqb dev` screen: step lines, problem blocks, the ready box and event lines.

use std::time::Duration;

use crate::cli_text::models::CliStyle;

/// What the terminal supports and how much room the screen has.
#[derive(Clone, Copy, Debug)]
pub struct ScreenFacts {
    /// The terminal's width; live rows are cut one cell short of it, and lasting text wraps there.
    pub columns: usize,
    /// The terminal's height; the live rows never use more than one row less.
    pub rows: usize,
    /// Colours, or none in plain mode.
    pub style: CliStyle,
    /// Whether to wrap locations in OSC 8 hyperlinks.
    pub links: bool,
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

/// The state glyph a step line starts with.
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

/// One step line: `✓ Compiled project        2.4s   0 errors`.
#[derive(Clone, Debug)]
pub struct StepLine {
    pub state: PhaseState,
    pub label: String,
    pub elapsed: Option<Duration>,
    pub detail: String,
}

/// How serious a problem block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemLevel {
    Error,
    Warning,
}

/// A problem block: a title line with its code, then `at`, `why` and `fix`, each wrapped in full.
#[derive(Clone, Debug)]
pub struct Problem {
    pub level: ProblemLevel,
    pub title: String,
    /// Right-aligned on the title line, such as `D001` or `snowflake`; empty for none.
    pub code: String,
    /// Where it happened, one entry per line; empty for none.
    pub at: Vec<Segment>,
    pub why: String,
    pub fix: String,
}

/// The first line of the `sqb dev` screen.
#[derive(Clone, Debug)]
pub struct DevHeader {
    pub version: String,
    pub project: String,
    /// Empty while the configuration has not named one.
    pub target: String,
}

/// Whether the dev target's warehouse connection is open.
#[derive(Clone, Debug)]
pub enum WarehouseState {
    /// Connected; `facts` are labelled settings such as `account xy12345` or `auth key pair`.
    Connected {
        facts: Vec<String>,
    },
    /// Not connected, and no background step will prompt for sign-in.
    NotConnected {
        reason: String,
    },
    Failed {
        message: String,
    },
}

/// One labelled row of the ready box; its parts are joined with ` · ` and wrap between parts.
#[derive(Clone, Debug)]
pub struct BoxRow {
    pub label: String,
    pub parts: Vec<String>,
    /// Makes the whole value clickable.
    pub link: Option<String>,
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

/// One timestamped event line of the `sqb dev` screen, with indented notes below it.
#[derive(Clone, Debug)]
pub struct EventLine {
    /// The local time, `HH:MM:SS`.
    pub clock: String,
    pub mark: EventMark,
    pub title: String,
    pub detail: String,
    pub elapsed: Option<Duration>,
    pub notes: Vec<String>,
}

/// One key of the key line.
#[derive(Clone, Debug)]
pub struct Hotkey {
    pub key: char,
    pub label: String,
}

/// An interactive prompt from the warehouse client, shown above the live steps.
#[derive(Clone, Debug)]
pub struct PromptBlock {
    pub title: String,
    /// The client's own text as it wrote it, including what the person typed.
    pub text: String,
    pub waited: Duration,
}

/// The one line a session ends with, and the commands to try next.
#[derive(Clone, Debug)]
pub struct ResultLine {
    pub failed: bool,
    pub title: String,
    pub detail: String,
    pub elapsed: Option<Duration>,
    /// `(command, what it does)` pairs under `Next`.
    pub next: Vec<(String, String)>,
}
