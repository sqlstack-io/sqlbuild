//! A phase's completion message as a phase line: `Compiled project. (0.81s)` as `✓ Project compiled  (0.81s)`.

use crate::cli_text::main::format_phase_line::format_phase_line;
use crate::cli_text::models::CliStyle;
use crate::progress::_helpers::completion::{completion_label, completion_parts};
use crate::progress::main::is_planning_completion_message::is_planning_completion_message;

/// `✓ <What> <verb>  <detail>  (<time>)` for a `<Verb> <what>[: <detail>]. (<time>)` completion message; `None` for any other text.
#[must_use]
pub fn completion_phase_line(message: &str, style: CliStyle) -> Option<String> {
    let message: &str = message.trim();
    if !is_planning_completion_message(message) {
        return None;
    }
    let (verb, subject, summary) = completion_parts(message)?;
    let label: String = completion_label(verb, subject)?;
    Some(format_phase_line(style, true, (&label, &summary)))
}
