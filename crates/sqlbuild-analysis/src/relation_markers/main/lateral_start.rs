//! Where a `LATERAL` keyword stands just before a relation marker call.

use crate::relation_markers::constants::LATERAL_KEYWORD;

/// The start of a whole-word `LATERAL` in code right before `start`, past whitespace only.
pub fn lateral_start(
    sql: &str,
    start: usize,
    mut is_code: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let before: &str = sql[..start].trim_end();
    let keyword_start: usize = before.len().checked_sub(LATERAL_KEYWORD.len())?;
    (before.is_char_boundary(keyword_start)
        && before[keyword_start..].eq_ignore_ascii_case(LATERAL_KEYWORD)
        && before[..keyword_start]
            .chars()
            .next_back()
            .is_none_or(|character| {
                !(character.is_alphanumeric() || matches!(character, '_' | '$'))
            })
        && is_code(keyword_start))
    .then_some(keyword_start)
}
