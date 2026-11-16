//! Redraw a block of rows in place.

use std::fmt::Write;

use crate::screen_text::constants::{CLEAR_BELOW, CURSOR_UP, CURSOR_UP_END};

/// Move up over the `previous` rows, clear them, and draw `rows`, each already fitting one line.
#[must_use]
pub fn redraw_rows(previous: usize, rows: &[String]) -> String {
    let mut text: String = String::new();
    if previous > 0 {
        let _ = write!(text, "{CURSOR_UP}{previous}{CURSOR_UP_END}");
    }
    text.push_str(CLEAR_BELOW);
    for row in rows {
        text.push_str(row);
        text.push('\n');
    }
    text
}
