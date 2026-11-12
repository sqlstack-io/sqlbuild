//! Python's `substitute_placeholder_defaults`.

use sqlbuild_core::text::main::is_word_char::is_word_char;

use crate::constants::PLACEHOLDER_PREFIX;

/// `substitute_placeholder_defaults`: `@@@name` becomes its default; unknown names stay.
pub fn placeholder_defaults(sql: &str, placeholders: &[(String, String)]) -> String {
    let mut result: String = String::with_capacity(sql.len());
    let mut copied: usize = 0;
    for (start, end, default) in placeholder_default_edits(sql, placeholders) {
        result.push_str(&sql[copied..start]);
        result.push_str(default);
        copied = end;
    }
    result.push_str(&sql[copied..]);
    result
}

/// The byte range of each `@@@name` with a default and that default, in order.
pub fn placeholder_default_edits<'a>(
    sql: &str,
    placeholders: &'a [(String, String)],
) -> Vec<(usize, usize, &'a str)> {
    let mut edits: Vec<(usize, usize, &'a str)> = Vec::new();
    let mut position: usize = 0;
    while let Some(found) = sql[position..].find(PLACEHOLDER_PREFIX) {
        let start: usize = position + found;
        let after: &str = &sql[start + PLACEHOLDER_PREFIX.len()..];
        let length: usize = after
            .find(|character: char| !is_word(character))
            .unwrap_or(after.len());
        if length == 0 {
            position = start + 1;
            continue;
        }
        let end: usize = start + PLACEHOLDER_PREFIX.len() + length;
        let name: &str = &after[..length];
        if let Some((_, default)) = placeholders
            .iter()
            .find(|(placeholder, _)| placeholder == name)
        {
            edits.push((start, end, default.as_str()));
        }
        position = end;
    }
    edits
}

/// A character Python's `\w` matches.
fn is_word(character: char) -> bool {
    is_word_char(character)
}
