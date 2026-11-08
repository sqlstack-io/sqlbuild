//! Whether SQL text ends inside an open `TABLE(` wrapper.

use crate::sql_scan::constants::TABLE_WRAPPER;

/// Whether `before`, ignoring trailing ASCII whitespace, ends with the word `TABLE` and `(`.
#[must_use]
pub fn follows_table_wrapper(before: &[u8]) -> bool {
    let Some(before) = before.trim_ascii_end().strip_suffix(b"(") else {
        return false;
    };
    let before = before.trim_ascii_end();
    let keyword = &TABLE_WRAPPER.as_bytes()[..TABLE_WRAPPER.len() - 1];
    let Some(start) = before.len().checked_sub(keyword.len()) else {
        return false;
    };
    before[start..].eq_ignore_ascii_case(keyword)
        && start.checked_sub(1).is_none_or(|index| {
            !(before[index].is_ascii_alphanumeric() || matches!(before[index], b'_' | b'$'))
        })
}
