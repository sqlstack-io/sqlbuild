//! CTE-list SQL macro calls in the `WITH` list of a macro-mode SQL test.

pub(crate) fn leading_space(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// Whether the text after a call ends the `WITH` list: nothing, a statement end or `SELECT`.
pub(crate) fn ends_list(text: &str) -> bool {
    let rest: &str = text.trim_start();
    let word: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    rest.is_empty() || rest.starts_with(';') || word.eq_ignore_ascii_case("SELECT")
}

/// Whether byte `offset` falls in one of the blanked spans.
pub(crate) fn is_blanked(offset: usize, blanked: &[(usize, usize)]) -> bool {
    blanked
        .iter()
        .any(|(start, end)| *start <= offset && offset < *end)
}
