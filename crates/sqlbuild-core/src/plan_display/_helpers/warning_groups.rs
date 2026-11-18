//! Plan warnings grouped by message template, so one message about many names reads once.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::plan_display::_helpers::plan_pages::text;
use crate::plan_display::constants::{ELIDED_NAME, QUOTE_CHARACTERS};

/// One template and its warnings in plan order.
pub(crate) struct WarningGroup<'document> {
    pub(crate) template: String,
    pub(crate) warnings: Vec<&'document Value>,
}

/// Warnings grouped by template, largest group first and groups of a size in the order they first appear.
pub(crate) fn warning_groups<'document>(
    warnings: &[&'document Value],
) -> Vec<WarningGroup<'document>> {
    let mut groups: Vec<WarningGroup<'document>> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for warning in warnings {
        let template = warning_template(warning);
        let at = *index.entry(template.clone()).or_insert_with(|| {
            groups.push(WarningGroup {
                template,
                warnings: Vec::new(),
            });
            groups.len() - 1
        });
        groups[at].warnings.push(warning);
    }
    groups.sort_by_key(|group| std::cmp::Reverse(group.warnings.len()));
    groups
}

/// A group as the summary lists it: its template, size, and the things it names when they repeat.
pub(crate) fn group_json(group: &WarningGroup<'_>) -> Value {
    json!({
        "template": group.template,
        "count": group.warnings.len(),
        "subject": subject(group),
    })
}

/// A template opening with a noun, such as `scenario …`, whose warnings name fewer of those than they number.
fn subject(group: &WarningGroup<'_>) -> Value {
    let noun = group
        .template
        .split_once(" …")
        .map(|(noun, _)| noun)
        .filter(|noun| !noun.is_empty() && noun.chars().all(is_word));
    let named: Option<HashSet<&str>> = group
        .warnings
        .iter()
        .map(|warning| named(warning))
        .collect();
    match (noun, named) {
        (Some(noun), Some(named))
            if group.warnings.len() > 1 && named.len() < group.warnings.len() =>
        {
            json!({"noun": noun.to_lowercase(), "count": named.len()})
        }
        _ => Value::Null,
    }
}

/// The message without its leading model name, with quoted names and generated-looking names elided.
fn warning_template(warning: &Value) -> String {
    let message = text(warning, "message");
    let model = text(warning, "model_name");
    let message = message
        .strip_prefix(model)
        .and_then(|rest| rest.strip_prefix(':'))
        .filter(|_| !model.is_empty())
        .map_or(message, str::trim);
    let template = elide_generated(&elide_quoted(message));
    match template.strip_suffix('.') {
        Some(rest) if rest.ends_with(ELIDED_NAME) => rest.to_owned(),
        _ => template,
    }
}

/// Each `'…'`, `` `…` `` or `"…"` span as one elision; an unclosed quote stays.
fn elide_quoted(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(character) = rest.chars().next() {
        let after = &rest[character.len_utf8()..];
        if QUOTE_CHARACTERS.contains(&character)
            && let Some(end) = after.find(character)
        {
            out.push_str(ELIDED_NAME);
            rest = &after[end + character.len_utf8()..];
        } else {
            out.push(character);
            rest = after;
        }
    }
    out
}

/// Each word with a digit or a double underscore, as generated names have, as one elision.
fn elide_generated(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut word = String::new();
    for character in message.chars().chain(std::iter::once(' ')) {
        if is_word(character) {
            word.push(character);
            continue;
        }
        if word.contains("__") || word.chars().any(|letter| letter.is_ascii_digit()) {
            out.push_str(ELIDED_NAME);
        } else {
            out.push_str(&word);
        }
        word.clear();
        out.push(character);
    }
    out.pop();
    out
}

/// The nonempty name a warning's message quotes first.
fn named(warning: &Value) -> Option<&str> {
    first_quoted(text(warning, "message")).filter(|name| !name.is_empty())
}

/// The text inside the message's first quoted span.
fn first_quoted(message: &str) -> Option<&str> {
    let (start, quote) = message.char_indices().find(|(at, character)| {
        QUOTE_CHARACTERS.contains(character)
            && message[at + character.len_utf8()..].contains(*character)
    })?;
    let inner = &message[start + quote.len_utf8()..];
    inner.split(quote).next()
}

const fn is_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}
