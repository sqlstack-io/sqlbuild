//! The glyphs, widths and escapes of live terminal screens.

pub(crate) const OK_GLYPH: &str = "\u{2713}";
pub(crate) const WARN_GLYPH: &str = "\u{26a0}";
pub(crate) const FAIL_GLYPH: &str = "\u{2717}";
pub(crate) const PENDING_GLYPH: &str = "\u{b7}";
pub(crate) const CHANGED_GLYPH: &str = "~";
pub(crate) const RESTARTED_GLYPH: &str = "\u{21bb}";
pub(crate) const STARTED_GLYPH: &str = "\u{25b6}";
pub(crate) const CARD_GLYPH: &str = "\u{279c}";
pub(crate) const CONNECTED_GLYPH: &str = "\u{25cf}";
pub(crate) const DISCONNECTED_GLYPH: &str = "\u{25cb}";
pub(crate) const SPINNER_FRAMES: [&str; 10] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
    "\u{2807}", "\u{280f}",
];
/// What replaces the cut tail of a row.
pub(crate) const CUT_MARK: &str = "\u{2026}";
pub(crate) const SEPARATOR: &str = " \u{b7} ";
pub(crate) const RULE_CHAR: char = '\u{2500}';
pub(crate) const TREE_BRANCH: &str = "\u{251c}\u{2500}\u{2500} ";
pub(crate) const TREE_LAST: &str = "\u{2514}\u{2500}\u{2500} ";
pub(crate) const TREE_BAR: &str = "\u{2502}   ";
pub(crate) const TREE_SPACE: &str = "    ";
pub(crate) const PRODUCT: &str = "sqlbuild dev";
pub(crate) const PLAIN_PREFIX: &str = "sqlbuild dev: ";
/// The label column of a phase line.
pub(crate) const PHASE_LABEL_WIDTH: usize = 20;
/// The right-aligned time column of phase and event lines.
pub(crate) const TIME_WIDTH: usize = 8;
pub(crate) const EVENT_CLOCK_WIDTH: usize = 8;
pub(crate) const EVENT_TITLE_WIDTH: usize = 32;
pub(crate) const EVENT_DETAIL_WIDTH: usize = 18;
pub(crate) const CARD_LABEL_WIDTH: usize = 12;
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
