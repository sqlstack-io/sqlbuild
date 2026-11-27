//! The parts of a `<Verb> <what>[: <detail>]. (<time>)` completion message, and its phase-line label.

use crate::progress::constants::{IRREGULAR_PARTICIPLES, LOWERCASE_NAMES, UNMOVABLE_SUBJECT_MARKS};

/// The verb, the subject before any `: <detail>`, and the summary (detail, then time); `None` for a subject with clauses of its own.
pub(crate) fn completion_parts(message: &str) -> Option<(&str, &str, String)> {
    let (verb, rest) = message.split_once(' ')?;
    let (subject, time) = match rest.rsplit_once(" (") {
        Some((subject, time)) if time.ends_with(')') => (subject, format!("({time}")),
        _ => (rest, String::new()),
    };
    let subject: &str = subject.trim().trim_end_matches('.');
    let (subject, detail) = subject.split_once(": ").unwrap_or((subject, ""));
    if subject.contains(UNMOVABLE_SUBJECT_MARKS) {
        return None;
    }
    let summary: String = [detail, time.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("  ");
    Some((verb, subject, summary))
}

/// `<Subject> <participle>`: the subject capitalised unless it leads with a lowercase name such as dbt.
pub(crate) fn completion_label(verb: &str, subject: &str) -> Option<String> {
    let mut characters = subject.chars();
    let first: char = characters.next()?;
    let leading_name: bool =
        LOWERCASE_NAMES.contains(&subject.split(' ').next().unwrap_or_default());
    let first: String = match leading_name {
        true => first.to_string(),
        false => first.to_uppercase().to_string(),
    };
    Some(format!(
        "{first}{} {}",
        characters.as_str(),
        participle(verb)
    ))
}

/// The participle a phase line puts after its subject: the verb itself, or its irregular form.
fn participle(verb: &str) -> String {
    IRREGULAR_PARTICIPLES
        .iter()
        .find(|(irregular, _)| *irregular == verb)
        .map_or_else(|| verb.to_lowercase(), |(_, form)| (*form).to_owned())
}
