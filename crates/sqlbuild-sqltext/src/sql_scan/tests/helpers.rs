//! Shared lexical syntaxes for SQL scanning tests.

use crate::sql_scan::_helpers::dollar::dollar_quote_end;
use crate::sql_scan::main::comment_end::comment_end;
use crate::sql_scan::main::quote_end::quote_end;
use crate::sql_scan::models::{LexicalSyntax, QuotePolicy, Unclosed};
use crate::sql_scan::tests::test_types::DispatchTestCase;

// The pre-optimization comment-first dispatch is the differential oracle. Keep it
// independent of non_code_end so its first-byte dispatch is what the tests exercise.
fn comment_first_end(
    sql: &[u8],
    index: usize,
    policy: QuotePolicy,
) -> Result<Option<usize>, Unclosed> {
    comment_end(sql, index)?.map_or_else(
        || {
            sql.get(index)
                .filter(|byte| policy.is_quote(**byte))
                .map_or_else(
                    || {
                        sql.get(index)
                            .filter(|byte| **byte == b'$' && policy.dollar_quotes)
                            .map_or(Ok(None), |_| dollar_quote_end(sql, index))
                    },
                    |_| quote_end(sql, index, policy).map(Some),
                )
        },
        |end| Ok(Some(end)),
    )
}

pub(crate) fn dispatch_truncation_cases(input: &[u8]) -> Vec<DispatchTestCase> {
    let mut cases = Vec::new();
    for flags in 0..16 {
        let policy = QuotePolicy {
            backtick_identifiers: flags & 1 != 0,
            single_quote_backslash_escapes: flags & 2 != 0,
            double_quote_backslash_escapes: flags & 4 != 0,
            dollar_quotes: flags & 8 != 0,
        };
        // The byte API must also handle truncations inside a multibyte character.
        for length in 0..=input.len() {
            let sql = &input[..length];
            for index in 0..=length + 1 {
                cases.push(DispatchTestCase {
                    description: format!(
                        "input={input:?}, length={length}, index={index}, policy={policy:?}"
                    ),
                    sql: sql.to_vec(),
                    index,
                    policy,
                    expected_end: comment_first_end(sql, index, policy),
                });
            }
        }
    }
    cases
}

pub(crate) fn ordinary_byte_cases(suffix: &[u8]) -> Vec<DispatchTestCase> {
    let mut cases = Vec::new();
    for byte in
        (u8::MIN..=u8::MAX).filter(|byte| !matches!(byte, b'-' | b'/' | b'\'' | b'"' | b'`' | b'$'))
    {
        for policy in [QuotePolicy::COMPILER, QuotePolicy::SQL_LINT] {
            let mut sql = vec![byte];
            sql.extend_from_slice(suffix);
            cases.push(DispatchTestCase {
                description: format!("byte={byte}, suffix={suffix:?}, policy={policy:?}"),
                sql,
                index: 0,
                policy,
                expected_end: Ok(None),
            });
        }
    }
    cases
}

pub(crate) fn generic() -> LexicalSyntax {
    LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    }
}

pub(crate) fn single_quote_backslashes() -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_string()],
        line_comment_prefixes: vec!["--".to_string(), "//".to_string()],
        ..LexicalSyntax::default()
    }
}

pub(crate) fn escape_strings() -> LexicalSyntax {
    LexicalSyntax {
        escape_string_prefix: true,
        nested_block_comments: true,
        ..generic()
    }
}

pub(crate) fn triple_quotes() -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_string(), "\"".to_string()],
        triple_quoted_strings: true,
        line_comment_prefixes: vec!["--".to_string(), "#".to_string()],
        ..LexicalSyntax::default()
    }
}

pub(crate) fn raw_strings() -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_string()],
        raw_string_prefix: true,
        ..generic()
    }
}

/// Lexical rules with the given quotes, `[escape, raw, triple, nested]` flags and prefixes.
pub(crate) fn syntax(quotes: &[&str], flags: [bool; 4], prefixes: &[&str]) -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: quotes.iter().map(|quote| (*quote).to_owned()).collect(),
        escape_string_prefix: flags[0],
        raw_string_prefix: flags[1],
        triple_quoted_strings: flags[2],
        nested_block_comments: flags[3],
        line_comment_prefixes: prefixes.iter().map(|prefix| (*prefix).to_owned()).collect(),
        bracket_identifiers: false,
    }
}
