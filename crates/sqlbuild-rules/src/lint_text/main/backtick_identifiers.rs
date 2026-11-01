/// Return whether SQL lint treats backticks as identifier quotes for `dialect`.
pub fn backtick_identifiers(dialect: &str) -> bool {
    crate::lint_text::_helpers::preparation::backtick_identifiers(dialect)
}
