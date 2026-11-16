//! One adapter's `SqlLexicalSyntax`, read from a Python mapping.

use crate::sqltext::models::LexicalSyntaxInput;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

impl From<LexicalSyntaxInput> for LexicalSyntax {
    fn from(input: LexicalSyntaxInput) -> Self {
        Self {
            backslash_escape_quotes: input.backslash_escape_quotes,
            escape_string_prefix: input.escape_string_prefix,
            raw_string_prefix: input.raw_string_prefix,
            triple_quoted_strings: input.triple_quoted_strings,
            nested_block_comments: input.nested_block_comments,
            line_comment_prefixes: input.line_comment_prefixes,
            bracket_identifiers: false,
        }
    }
}
