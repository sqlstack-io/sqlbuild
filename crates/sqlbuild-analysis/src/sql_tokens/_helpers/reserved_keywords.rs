//! Per-dialect reserved keywords, which can never be unquoted identifiers in that dialect.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::sql_tokens::models::KeywordList;

static DUCKDB: LazyLock<HashSet<String>> =
    LazyLock::new(|| keyword_set(include_str!("../reserved_keywords/duckdb.txt")));
static POSTGRES: LazyLock<HashSet<String>> =
    LazyLock::new(|| keyword_set(include_str!("../reserved_keywords/postgres.txt")));
static SNOWFLAKE: LazyLock<HashSet<String>> =
    LazyLock::new(|| keyword_set(include_str!("../reserved_keywords/snowflake.txt")));
static GENERIC: LazyLock<HashSet<String>> =
    LazyLock::new(|| keyword_set(include_str!("../reserved_keywords/generic.txt")));

/// Whether the upper-cased word is reserved in `list`.
pub(crate) fn is_reserved_keyword(list: KeywordList, upper_word: &str) -> bool {
    let keywords: &HashSet<String> = match list {
        KeywordList::DuckDB => &DUCKDB,
        KeywordList::PostgreSQL => &POSTGRES,
        KeywordList::Snowflake => &SNOWFLAKE,
        KeywordList::Generic => &GENERIC,
    };
    keywords.contains(upper_word)
}

fn keyword_set(data: &str) -> HashSet<String> {
    data.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_ascii_uppercase)
        .collect()
}
