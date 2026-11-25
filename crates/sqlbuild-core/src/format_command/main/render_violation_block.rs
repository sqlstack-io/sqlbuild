//! One lint finding as a located diagnostic block.

use crate::cli_text::models::CliStyle;
use crate::format_command::_helpers::diagnostics::violation_block;
use crate::format_command::models::LintFinding;

/// The finding's header, `-->` place, source line and caret, in the lint report's layout.
#[must_use]
pub fn render_violation_block(finding: &LintFinding, style: CliStyle) -> String {
    violation_block(finding, style)
}
