//! A phase's completion message as a phase line: `Compiled project. (0.81s)` as `✓ Project compiled  (0.81s)`.

use crate::cli_text::main::format_phase_line::format_phase_line;
use crate::cli_text::models::CliStyle;
use crate::progress::constants::{IRREGULAR_PARTICIPLES, LOWERCASE_NAMES};
use crate::progress::main::is_planning_completion_message::is_planning_completion_message;

/// `✓ <What> <verb>  <detail>  (<time>)` for a `<Verb> <what>[: <detail>]. (<time>)` completion message; `None` for any other text.
#[must_use]
pub fn completion_phase_line(message: &str, style: CliStyle) -> Option<String> {
    let message: &str = message.trim();
    if !is_planning_completion_message(message) {
        return None;
    }
    let (verb, rest) = message.split_once(' ')?;
    let (what, time) = match rest.rsplit_once(" (") {
        Some((what, time)) if time.ends_with(')') => (what, format!("({time}")),
        _ => (rest, String::new()),
    };
    let what: &str = what.trim().trim_end_matches('.');
    // `Checked relation usage: 2 of 4 in use.` keeps what it checked as the label and the rest as detail.
    let (what, detail) = what.split_once(": ").unwrap_or((what, ""));
    let summary: String = [detail, time.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("  ");
    let leading_name: bool = LOWERCASE_NAMES.contains(&what.split(' ').next().unwrap_or_default());
    let mut characters = what.chars();
    let first: char = characters.next()?;
    let participle: String = IRREGULAR_PARTICIPLES
        .iter()
        .find(|(irregular, _)| *irregular == verb)
        .map_or_else(
            || verb.to_lowercase(),
            |(_, participle)| (*participle).to_owned(),
        );
    let label: String = format!(
        "{}{} {participle}",
        if leading_name {
            first.to_string()
        } else {
            first.to_uppercase().to_string()
        },
        characters.as_str(),
    );
    Some(format_phase_line(style, true, (&label, &summary)))
}
