//! A diagnostic's help text as `= help:` lines.

use crate::cli_text::models::CliStyle;
use crate::format_command::_helpers::diagnostics::help_lines;

/// The help text wrapped under a `= help:` label, continuation lines aligned under the text.
#[must_use]
pub fn render_help_lines(help: &str, style: CliStyle) -> Vec<String> {
    help_lines(help, style)
}
