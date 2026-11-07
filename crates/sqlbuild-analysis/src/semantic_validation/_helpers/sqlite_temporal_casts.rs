//! `CAST(... AS TIMESTAMP)` and its date and time siblings on SQLite, which keep only a number.

use crate::semantic_validation::_helpers::function_names::{Cursor, is_word_byte};
use polyglot_sql::{DialectType, ValidationError};
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::models::{QuotePolicy, Unclosed};

pub(crate) const SQLITE_TEMPORAL_CAST_CODE: &str = "B104";
const CAST_KEYWORD: &str = "CAST";
const AS_KEYWORD: &str = "AS";
const TEMPORAL_TYPES: [&str; 5] = ["TIMESTAMP", "TIMESTAMPTZ", "DATETIME", "DATE", "TIME"];

/// One authored `CAST(... AS <temporal type>)`, spanning `CAST` through the type name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TemporalCast {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) type_name: String,
}

/// Located B104 warnings for temporal casts, which SQLite applies with NUMERIC affinity.
pub(crate) fn sqlite_temporal_cast_warnings(
    sql: &str,
    dialect: DialectType,
) -> Vec<ValidationError> {
    if dialect != DialectType::SQLite {
        return Vec::new();
    }
    let mut cursor: Cursor = Cursor::new();
    temporal_casts(sql)
        .into_iter()
        .map(|cast| {
            let message: String = format!(
                "SQLite casts to {} with NUMERIC affinity and keeps only the leading number, so \
                 '2026-01-01 10:00:00' becomes 2026",
                cast.type_name
            );
            cursor.locate(
                sql,
                (cast.start, cast.end),
                ValidationError::warning(message, SQLITE_TEMPORAL_CAST_CODE),
            )
        })
        .collect()
}

/// Every top-level `AS <temporal type>` inside a `CAST(...)`, outside comments and quoted text.
pub(crate) fn temporal_casts(sql: &str) -> Vec<TemporalCast> {
    let bytes: &[u8] = sql.as_bytes();
    let mut casts: Vec<TemporalCast> = Vec::new();
    let mut open_casts: Vec<(usize, usize)> = Vec::new();
    let mut depth: usize = 0;
    let mut pending_cast: Option<usize> = None;
    let mut after_as: Option<usize> = None;
    let mut index: usize = 0;
    while index < bytes.len() {
        let byte: u8 = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        match non_code_end(bytes, index, QuotePolicy::COMPILER) {
            Ok(Some(end)) => {
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => break,
        }
        if byte == b'(' {
            depth += 1;
            if let Some(start) = pending_cast.take() {
                open_casts.push((start, depth));
            }
            after_as = None;
            index += 1;
            continue;
        }
        pending_cast = None;
        if byte == b')' {
            if open_casts.last().is_some_and(|(_, level)| *level == depth) {
                open_casts.pop();
            }
            depth = depth.saturating_sub(1);
            after_as = None;
            index += 1;
            continue;
        }
        if !is_word_byte(byte) {
            after_as = None;
            index += 1;
            continue;
        }
        let word_start: usize = index;
        while index < bytes.len() && is_word_byte(bytes[index]) {
            index += 1;
        }
        let word: &str = &sql[word_start..index];
        if let Some(cast_start) = after_as.take() {
            let upper: String = word.to_ascii_uppercase();
            if TEMPORAL_TYPES.contains(&upper.as_str()) {
                casts.push(TemporalCast {
                    start: cast_start,
                    end: index,
                    type_name: upper,
                });
            }
            continue;
        }
        if word.eq_ignore_ascii_case(CAST_KEYWORD) {
            pending_cast = Some(word_start);
        } else if word.eq_ignore_ascii_case(AS_KEYWORD)
            && let Some((cast_start, _)) = open_casts.last().filter(|(_, level)| *level == depth)
        {
            after_as = Some(*cast_start);
        }
    }
    casts
}
