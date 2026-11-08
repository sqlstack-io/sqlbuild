//! Whether one dialect's lexical rules can read a SQL text differently from generic SQL.

use crate::sql_scan::constants::{
    BLOCK_COMMENT_OPEN, ESCAPE_CHARACTER, GENERIC_LINE_COMMENT_PREFIX, TRIPLE_QUOTES,
};
use crate::sql_scan::models::LexicalSyntax;

/// `SqlLexicalSyntax.reads_differently_from_generic`.
#[must_use]
pub fn reads_differently_from_generic(syntax: &LexicalSyntax, sql: &str) -> bool {
    if sql.contains(ESCAPE_CHARACTER)
        && (!syntax.backslash_escape_quotes.is_empty() || syntax.escape_string_prefix)
    {
        return true;
    }
    if syntax.triple_quoted_strings && TRIPLE_QUOTES.iter().any(|quote| sql.contains(quote)) {
        return true;
    }
    if syntax.nested_block_comments && sql.matches(BLOCK_COMMENT_OPEN).count() > 1 {
        return true;
    }
    if syntax.bracket_identifiers && sql.contains('[') {
        return true;
    }
    syntax
        .line_comment_prefixes
        .iter()
        .any(|prefix| prefix != GENERIC_LINE_COMMENT_PREFIX && sql.contains(prefix.as_str()))
}
