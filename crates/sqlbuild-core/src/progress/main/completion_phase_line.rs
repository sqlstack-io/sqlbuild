//! A phase's completion message as a phase line: `Compiled project. (0.81s)` as `✓ Project compiled  (0.81s)`.

use crate::cli_text::main::format_phase_line::format_phase_line;
use crate::cli_text::models::CliStyle;
use crate::progress::main::is_planning_completion_message::is_planning_completion_message;

/// `✓ <What> <verb>  (<time>)` for a `<Verb> <what>. (<time>)` completion message; `None` for any other text.
#[must_use]
pub fn completion_phase_line(message: &str, style: CliStyle) -> Option<String> {
    let message: &str = message.trim();
    if !is_planning_completion_message(message) {
        return None;
    }
    let (verb, rest) = message.split_once(' ')?;
    let (what, summary) = match rest.rsplit_once(" (") {
        Some((what, time)) if time.ends_with(')') => (what, format!("({time}")),
        _ => (rest, String::new()),
    };
    let what: &str = what.trim().trim_end_matches('.');
    let mut characters = what.chars();
    let first: char = characters.next()?;
    let label: String = format!(
        "{}{} {}",
        first.to_uppercase(),
        characters.as_str(),
        verb.to_lowercase()
    );
    Some(format_phase_line(style, true, (&label, &summary)))
}
