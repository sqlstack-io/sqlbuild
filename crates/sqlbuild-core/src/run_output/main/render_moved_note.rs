//! The note after the rows of an `auto` run about items that moved to the warehouse while running.

use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::causes::cause_list;
use crate::run_output::models::HeaderCause;

/// A blank line, then `  note: N tests moved to the warehouse while running: <causes> (--verbose for why)`.
#[must_use]
pub fn render_moved_note(
    moved: usize,
    causes: &[HeaderCause],
    (singular, plural): (&str, &str),
    style: CliStyle,
) -> String {
    let noun: &str = if moved == 1 { singular } else { plural };
    format!(
        "\n  {} {}\n",
        style.value("note:"),
        style.muted(&format!(
            "{moved} {noun} moved to the warehouse while running: {} (--verbose for why)",
            cause_list(causes)
        ))
    )
}
