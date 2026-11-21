//! The `OVER (...)` clause that follows a table function call.

use crate::resolution::constants::OVER_KEYWORD;
use crate::resolution::main::code_start::code_start;

/// The `(` of an `OVER (` clause after `start`, with whitespace or comments before each word.
#[must_use]
pub fn over_clause_open(sql: &str, start: usize, line_comments: &[String]) -> Option<usize> {
    let keyword: usize = code_start(sql, start, line_comments)?;
    let after: usize = keyword + OVER_KEYWORD.len();
    let word: &str = sql.get(keyword..after)?;
    if !word.eq_ignore_ascii_case(OVER_KEYWORD) {
        return None;
    }
    let open: usize = code_start(sql, after, line_comments)?;
    sql[open..].starts_with('(').then_some(open)
}
