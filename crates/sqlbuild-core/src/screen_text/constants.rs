//! The glyphs, widths and escapes of the `sqb dev` screen.

pub(crate) const OK_GLYPH: &str = "\u{2713}";
pub(crate) const WARN_GLYPH: &str = "\u{26a0}";
pub(crate) const FAIL_GLYPH: &str = "\u{2717}";
pub(crate) const PENDING_GLYPH: &str = "\u{b7}";
pub(crate) const PROMPT_GLYPH: &str = "\u{25cc}";
pub(crate) const CHANGED_GLYPH: &str = "~";
pub(crate) const RESTARTED_GLYPH: &str = "\u{21bb}";
pub(crate) const STARTED_GLYPH: &str = "\u{25b8}";
pub(crate) const SPINNER_FRAMES: [&str; 10] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
    "\u{2807}", "\u{280f}",
];
/// What replaces the cut tail of a live row.
pub(crate) const CUT_MARK: &str = "\u{2026}";
pub(crate) const SEPARATOR: &str = " \u{b7} ";
pub(crate) const RULE_CHAR: char = '\u{2500}';
pub(crate) const PRODUCT: &str = "sqb dev";
/// The label column of a step line.
pub(crate) const STEP_LABEL_WIDTH: usize = 22;
/// The right-aligned time column after a step's label.
pub(crate) const STEP_TIME_WIDTH: usize = 6;
/// The gap between a step's time and its detail.
pub(crate) const STEP_GAP: &str = "   ";
/// Where a problem block's `at`, `why` and `fix` text starts.
pub(crate) const PROBLEM_TEXT_COLUMN: usize = 9;
/// Where the first label of a problem block or prompt starts.
pub(crate) const PROBLEM_LABEL_INDENT: usize = 2;
/// The label column inside the ready box, such as `Warehouse  `.
pub(crate) const BOX_LABEL_WIDTH: usize = 11;
/// The ready box is this many cells narrower than the terminal.
pub(crate) const BOX_MARGIN: usize = 2;
/// The narrowest ready box drawn.
pub(crate) const BOX_MIN_WIDTH: usize = 24;
/// Where an event line's text starts, after `HH:MM:SS ✓ `.
pub(crate) const EVENT_TEXT_COLUMN: usize = 11;
/// The command column under `Next`.
pub(crate) const NEXT_COMMAND_WIDTH: usize = 28;
/// Lasting text never wraps narrower than this, however narrow the terminal.
pub(crate) const MIN_WRAP: usize = 20;
/// Times under a minute show tenths of a second.
pub(crate) const TENTHS_PER_MINUTE: u128 = 600;
pub(crate) const SECONDS_PER_MINUTE: u64 = 60;
pub(crate) const SECONDS_PER_HOUR: u64 = 3600;
pub(crate) const OSC8_START: &str = "\u{1b}]8;;";
pub(crate) const OSC8_END: &str = "\u{1b}\\";
/// Moves the cursor up `n` rows to column one, then clears to the end of the screen.
pub(crate) const CURSOR_UP: &str = "\u{1b}[";
pub(crate) const CURSOR_UP_END: &str = "F";
pub(crate) const CLEAR_BELOW: &str = "\u{1b}[J";
