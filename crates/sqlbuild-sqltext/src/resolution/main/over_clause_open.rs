//! The `OVER (...)` clause that follows a table function call.

use crate::resolution::constants::OVER_KEYWORD;

/// The byte index of the `(` of an `OVER (` clause after `start` and any whitespace, if one follows.
#[must_use]
pub fn over_clause_open(sql: &str, start: usize) -> Option<usize> {
    let tail: &str = sql.get(start..)?;
    let keyword: usize = start + tail.len() - tail.trim_start().len();
    let after: usize = keyword + OVER_KEYWORD.len();
    let word: &str = sql.get(keyword..after)?;
    if !word.eq_ignore_ascii_case(OVER_KEYWORD) {
        return None;
    }
    let rest: &str = &sql[after..];
    let open: usize = after + rest.len() - rest.trim_start().len();
    sql[open..].starts_with('(').then_some(open)
}
