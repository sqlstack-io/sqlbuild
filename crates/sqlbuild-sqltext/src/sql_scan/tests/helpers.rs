//! Shared lexical syntaxes for SQL scanning tests.

use crate::sql_scan::_helpers::dollar::dollar_quote_end;
use crate::sql_scan::main::bounded_expansions::bounded_expansions;
use crate::sql_scan::main::comment_end::comment_end;
use crate::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use crate::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
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

/// Negatives, comments, comment halves, operators, quotes, empty text and newlines.
const HOSTILE: [&str; 44] = [
    "",
    " ",
    "\n",
    "1",
    "-1",
    "- 1",
    "-",
    "--",
    "---",
    "-/",
    "/",
    "//",
    "/ 2",
    "*",
    "**",
    "*/",
    "/**/",
    "/* note */",
    "/* two\nlines */",
    "1 -- one",
    "1 -- one\n",
    "-- one",
    "-- one\n2",
    "1 // one",
    "//x",
    "#",
    "#x",
    "# x",
    "1 # one",
    "#!x",
    "!x",
    "+",
    "<",
    ">",
    "|",
    "'a--b'",
    "'/*'",
    "1-",
    "1/",
    "\n-1",
    "-1\n",
    "x -- a -- b",
    "1 /* c */ -- d",
    "\u{e9}-",
];
/// `{}` marks each site, all of them in code.
const POSITIONS: [&str; 25] = [
    "{}",
    "select {} as x, 7 as y",
    "select -{} as x, 7 as y",
    "select {}- 1",
    "select {}-1",
    "select a/{}",
    "select {}/2",
    "select {}*2",
    "select a/{}*2",
    "select {} -- note",
    "select {} /* note */",
    "select {} /* two\nlines */",
    "select {} /* note */ , b",
    "select {}\n, b",
    "select {}\r\n, b",
    "select {}  \n, b",
    "({})",
    "f({}, 2)",
    "{}_count",
    "select {}'text'",
    "select '--' || {}",
    "/* c */{}",
    "select {}{}",
    "select -{}-1, {}",
    "select {} {}\nfrom t",
];

/// `template` with `values` at its sites, as authored and once bounded.
pub(crate) fn spliced(template: &str, values: &[&str], syntax: &LexicalSyntax) -> (String, String) {
    let mut sql: String = String::new();
    let mut sites: Vec<(usize, usize, &str)> = Vec::new();
    let mut pieces = template.split("{}");
    sql.push_str(pieces.next().unwrap_or_default());
    for (piece, value) in pieces.zip(values) {
        sites.push((sql.len(), sql.len() + 2, value));
        sql.push_str("@v");
        sql.push_str(piece);
    }
    let mut texts: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
    for expansion in bounded_expansions(&sql, &sites, Some(syntax)) {
        assert_eq!(
            &expansion.text[expansion.lead..expansion.lead + values[expansion.index].len()],
            values[expansion.index]
        );
        texts[expansion.index] = expansion.text;
    }
    let joined = |texts: &[String]| {
        template
            .split("{}")
            .zip(texts.iter().map(String::as_str).chain([""]))
            .flat_map(|(piece, value)| [piece, value])
            .collect::<String>()
    };
    let raw: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
    (joined(&raw), joined(&texts))
}

/// The code and quoted text of `sql`, without whitespace or comments.
fn code(sql: &str, syntax: &LexicalSyntax) -> String {
    let mut kept: String = String::new();
    let mut at: usize = 0;
    for (start, end) in dialect_non_code_ranges(sql, syntax) {
        kept.extend(
            sql[at..start]
                .chars()
                .filter(|character| !character.is_whitespace()),
        );
        let construct: &str = &sql[start..end];
        kept.extend(Some(construct).filter(|text| text.starts_with(['\'', '"', '`'])));
        at = end;
    }
    kept.extend(
        sql[at..]
            .chars()
            .filter(|character| !character.is_whitespace()),
    );
    kept
}

/// Each splice whose bounded text reads wrong, or changed although it read right.
pub(crate) fn generated_failures(syntax: &LexicalSyntax) -> Vec<String> {
    POSITIONS
        .iter()
        .flat_map(|template| {
            HOSTILE.iter().flat_map(move |first| {
                HOSTILE
                    .iter()
                    .map(move |second| (*template, [*first, *second, *first]))
            })
        })
        .filter_map(|(template, values)| {
            let sites: usize = template.matches("{}").count();
            let values: &[&str] = &values[..sites];
            let expected: String = template
                .split("{}")
                .zip(values.iter().copied().chain([""]))
                .map(|(piece, value)| code(piece, syntax) + &code(value, syntax))
                .collect();
            let (raw, bounded) = spliced(template, values, syntax);
            let read_right: bool = code(&raw, syntax) == expected;
            let wrong: bool = code(&bounded, syntax) != expected || (read_right && bounded != raw);
            wrong.then(|| format!("{template:?} with {values:?} gave {bounded:?}"))
        })
        .collect()
}

/// Every text of up to `rounds` of `pieces` in a row.
pub(crate) fn piece_texts(pieces: &[&str], rounds: usize) -> Vec<String> {
    let mut texts: Vec<String> = vec![String::new()];
    for _ in 0..rounds {
        texts = texts
            .iter()
            .flat_map(|text| pieces.iter().map(move |piece| format!("{text}{piece}")))
            .chain(texts.iter().cloned())
            .collect();
    }
    texts
}

/// The non-code ranges of `sql` found by asking `dialect_non_code_end` about every byte in turn.
pub(crate) fn ranges_byte_by_byte(sql: &str, syntax: &LexicalSyntax) -> Vec<(usize, usize)> {
    let bytes: &[u8] = sql.as_bytes();
    let step = |index: usize| -> (usize, Option<(usize, usize)>) {
        dialect_non_code_end(bytes, index, syntax)
            .map_or((bytes.len(), Some((index, bytes.len()))), |end| {
                end.map_or((index + 1, None), |end| (end, Some((index, end))))
            })
    };
    std::iter::successors(Some((0, None)), |(index, _)| {
        (*index < bytes.len()).then(|| step(*index))
    })
    .filter_map(|(_, range)| range)
    .collect()
}
