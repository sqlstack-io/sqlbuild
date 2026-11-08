//! Truth-value tests (`IS [NOT] TRUE` / `IS [NOT] FALSE`) on dialects whose warehouse rejects them.

use crate::semantic_validation::_helpers::function_names::{Cursor, is_word_byte};
use polyglot_sql::{DialectType, ValidationError};
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::models::{QuotePolicy, Unclosed};

pub(crate) const UNSUPPORTED_TRUTH_TEST_CODE: &str = "B103";
const IS_KEYWORD: &str = "IS";
const NOT_KEYWORD: &str = "NOT";
const TRUE_KEYWORD: &str = "TRUE";
const FALSE_KEYWORD: &str = "FALSE";

/// One authored `IS [NOT] TRUE` or `IS [NOT] FALSE`, spanning `IS` through the truth value.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TruthTest {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) negated: bool,
    pub(crate) value: bool,
}

/// Return located B103 diagnostics for truth-value tests the dialect's warehouse rejects.
pub(crate) fn unsupported_truth_test_errors(
    sql: &str,
    dialect: DialectType,
) -> Vec<ValidationError> {
    if !rejects_truth_tests(dialect) {
        return Vec::new();
    }
    let mut cursor: Cursor = Cursor::new();
    truth_tests(sql)
        .into_iter()
        .map(|test| {
            let message: String = format!(
                "'{}' is not supported for dialect {dialect:?}; write {}",
                spelling(&test),
                rewrite(dialect, &test)
            );
            cursor.locate(
                sql,
                (test.start, test.end),
                ValidationError::error(message, UNSUPPORTED_TRUTH_TEST_CODE),
            )
        })
        .collect()
}

/// Whether the dialect's warehouse rejects `IS [NOT] TRUE` and `IS [NOT] FALSE`.
pub(crate) fn rejects_truth_tests(dialect: DialectType) -> bool {
    matches!(dialect, DialectType::Snowflake | DialectType::TSQL)
}

/// Return every `IS [NOT] TRUE|FALSE` keyword sequence outside comments and quoted text.
pub(crate) fn truth_tests(sql: &str) -> Vec<TruthTest> {
    let bytes: &[u8] = sql.as_bytes();
    let mut tests: Vec<TruthTest> = Vec::new();
    let mut pending: Option<(usize, bool)> = None;
    let mut index: usize = 0;
    while index < bytes.len() {
        let byte: u8 = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        match non_code_end(bytes, index, QuotePolicy::COMPILER) {
            Ok(Some(end)) => {
                if !matches!(byte, b'-' | b'/') {
                    pending = None;
                }
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => break,
        }
        if !is_word_byte(byte) {
            pending = None;
            index += 1;
            continue;
        }
        let start: usize = index;
        while index < bytes.len() && is_word_byte(bytes[index]) {
            index += 1;
        }
        let word: &str = &sql[start..index];
        pending = match pending {
            Some((is_start, false)) if word.eq_ignore_ascii_case(NOT_KEYWORD) => {
                Some((is_start, true))
            }
            Some((is_start, negated))
                if word.eq_ignore_ascii_case(TRUE_KEYWORD)
                    || word.eq_ignore_ascii_case(FALSE_KEYWORD) =>
            {
                tests.push(TruthTest {
                    start: is_start,
                    end: index,
                    negated,
                    value: word.eq_ignore_ascii_case(TRUE_KEYWORD),
                });
                None
            }
            _ => word
                .eq_ignore_ascii_case(IS_KEYWORD)
                .then_some((start, false)),
        };
    }
    tests
}

fn spelling(test: &TruthTest) -> String {
    format!(
        "{IS_KEYWORD} {}{}",
        if test.negated { "NOT " } else { "" },
        if test.value {
            TRUE_KEYWORD
        } else {
            FALSE_KEYWORD
        }
    )
}

/// The null-safe equivalent of one truth-value test in the dialect.
fn rewrite(dialect: DialectType, test: &TruthTest) -> &'static str {
    match (dialect, test.negated, test.value) {
        (DialectType::TSQL, false, true) => "COALESCE(<expr>, 0) = 1",
        (DialectType::TSQL, true, true) => "COALESCE(<expr>, 0) = 0",
        (DialectType::TSQL, false, false) => "COALESCE(<expr>, 1) = 0",
        (DialectType::TSQL, true, false) => "COALESCE(<expr>, 1) = 1",
        (_, false, true) => "COALESCE(<expr>, FALSE)",
        (_, true, true) => "NOT COALESCE(<expr>, FALSE) or <expr> IS DISTINCT FROM TRUE",
        (_, false, false) => "NOT COALESCE(<expr>, TRUE)",
        (_, true, false) => "COALESCE(<expr>, TRUE) or <expr> IS DISTINCT FROM FALSE",
    }
}
