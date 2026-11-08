//! `dialect_lexical_syntax`: the comment and quoted-text syntax of a dialect's SQL, by dialect name.

use crate::sql_scan::models::LexicalSyntax;

/// The comment and quoted-text syntax that SQLQuack tokenizes for `dialect`.
pub fn dialect_lexical_syntax(dialect: &str) -> LexicalSyntax {
    let strings = |quotes: &[&str]| quotes.iter().map(|quote| (*quote).to_owned()).collect();
    let lines = |prefixes: &[&str]| prefixes.iter().map(|prefix| (*prefix).to_owned()).collect();
    match dialect.to_ascii_lowercase().as_str() {
        "bigquery" => LexicalSyntax {
            backslash_escape_quotes: strings(&["'", "\""]),
            raw_string_prefix: true,
            triple_quoted_strings: true,
            line_comment_prefixes: lines(&["--", "#"]),
            ..LexicalSyntax::default()
        },
        "snowflake" => LexicalSyntax {
            backslash_escape_quotes: strings(&["'"]),
            line_comment_prefixes: lines(&["--", "//"]),
            ..LexicalSyntax::default()
        },
        "clickhouse" => LexicalSyntax {
            backslash_escape_quotes: strings(&["'"]),
            line_comment_prefixes: lines(&["--", "#", "//"]),
            ..LexicalSyntax::default()
        },
        "databricks" | "duckdb" => LexicalSyntax {
            line_comment_prefixes: lines(&["--"]),
            ..LexicalSyntax::default()
        },
        "mssql" | "sqlserver" | "tsql" => LexicalSyntax {
            nested_block_comments: true,
            line_comment_prefixes: lines(&["--"]),
            ..LexicalSyntax::default()
        },
        _ => LexicalSyntax {
            escape_string_prefix: true,
            nested_block_comments: true,
            line_comment_prefixes: lines(&["--"]),
            ..LexicalSyntax::default()
        },
    }
}
