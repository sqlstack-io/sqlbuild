//! A partitioned call's FROM-clause inputs read as one row of NULLs, so its fixture is read once.

use std::collections::BTreeMap;

use regex::Regex;

use crate::compiler::_helpers::sql_tests::markers::ProtectedRanges;
use crate::compiler::_helpers::sql_tests::planning::compile_error;
use crate::relation_markers::main::lateral_start::lateral_start;
use sqlbuild_sqltext::resolution::main::over_clause_open::over_clause_open;
use sqlbuild_sqltext::sql_scan::main::dialect_matching_paren::dialect_matching_paren;
use sqlbuild_sqltext::sql_scan::main::skip_whitespace::skip_whitespace;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

const FROM_KEYWORD: &str = "from";
const CROSS_JOIN: &str = " CROSS JOIN ";
const INPUTS_ALIAS: &str = "__sqb_partition_inputs";
const JOIN_WORDS: [&str; 8] = [
    "join", "cross", "inner", "left", "right", "full", "outer", "natural",
];

/// `sql` with each mocked partitioned call's inputs as `p RIGHT JOIN (SELECT 1) ON FALSE`.
pub(crate) fn collapse_partition_inputs(
    sql: &str,
    pattern: &Regex,
    syntax: &LexicalSyntax,
    mocked: impl Fn(&str) -> bool,
) -> Result<String, String> {
    let mut protected = ProtectedRanges::new(syntax, sql);
    let mut edits: BTreeMap<usize, (usize, String)> = BTreeMap::new();
    let mut collapsed = 0;
    for captures in pattern.captures_iter(sql) {
        let (Some(full), Some(name)) = (captures.get(0), captures.get(1)) else {
            continue;
        };
        if protected.contains(full.start()) || !mocked(name.as_str()) {
            continue;
        }
        let arguments = skip_whitespace(sql, full.end());
        if !sql[arguments..].starts_with('(') {
            continue;
        }
        let Ok(close) = dialect_matching_paren(sql.as_bytes(), arguments, syntax) else {
            continue;
        };
        let Some(over) = over_clause_open(sql, close + 1, &syntax.line_comment_prefixes) else {
            continue;
        };
        if protected.contains(over) {
            continue;
        }
        let call_start = lateral_start(sql, full.start(), |index| !protected.contains(index))
            .unwrap_or(full.start());
        let Some(scan) = scan_inputs(sql, call_start, &mut protected) else {
            return Err(compile_error(&format!(
                "a __table_fn__{} fixture replaces a call with an OVER clause, which must stand \
                 in a FROM clause",
                name.as_str()
            )));
        };
        let Some(separator) = scan.separator else {
            continue;
        };
        for comma in scan.commas {
            let end = skip_whitespace(sql, comma + 1);
            let _ = edits.entry(comma).or_insert((end, CROSS_JOIN.to_owned()));
        }
        let collapse = format!(
            " RIGHT JOIN (SELECT 1 AS {INPUTS_ALIAS}) AS {INPUTS_ALIAS}_{collapsed} ON FALSE"
        );
        collapsed += 1;
        match separator {
            Separator::Comma(comma) => {
                let end = skip_whitespace(sql, comma + 1);
                let _ = edits.insert(comma, (end, format!("{collapse}{CROSS_JOIN}")));
            }
            Separator::Join(start) => {
                let _ = edits.insert(start, (start, format!("{} ", collapse.trim_start())));
            }
        }
    }
    let mut output = String::with_capacity(sql.len());
    let mut cursor = 0;
    for (start, (end, text)) in edits {
        output.push_str(&sql[cursor..start]);
        output.push_str(&text);
        cursor = end;
    }
    output.push_str(&sql[cursor..]);
    Ok(output)
}

/// What joins a call to the relations before it.
enum Separator {
    /// The byte offset of the comma.
    Comma(usize),
    /// The start of the join keywords.
    Join(usize),
}

/// The relations before a call in its FROM clause.
struct Inputs {
    /// The top-level commas between `FROM` and the call.
    commas: Vec<usize>,
    /// What joins the call to them, or `None` when the call follows `FROM` directly.
    separator: Option<Separator>,
}

/// The inputs before `call_start`, back to its FROM keyword, or `None` outside a FROM clause.
fn scan_inputs(
    sql: &str,
    call_start: usize,
    protected: &mut ProtectedRanges<'_>,
) -> Option<Inputs> {
    let bytes = sql.as_bytes();
    let mut commas: Vec<usize> = Vec::new();
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut index = call_start;
    while index > 0 {
        index -= 1;
        if protected.contains(index) {
            continue;
        }
        match bytes[index] {
            b')' => depth += 1,
            b'(' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 => commas.push(index),
            byte if depth == 0 && is_word_byte(byte) => {
                let end = index + 1;
                while index > 0 && is_word_byte(bytes[index - 1]) && !protected.contains(index - 1)
                {
                    index -= 1;
                }
                if sql[index..end].eq_ignore_ascii_case(FROM_KEYWORD) {
                    return Some(Inputs {
                        separator: separator(sql, &commas, &words),
                        commas,
                    });
                }
                words.push((index, end));
            }
            _ => {}
        }
    }
    None
}

/// The separator right before the call, or `None` when the call follows `FROM` directly.
fn separator(sql: &str, commas: &[usize], words: &[(usize, usize)]) -> Option<Separator> {
    let last_comma = commas.first().copied();
    let mut join_start = None;
    for &(start, end) in words {
        if last_comma.is_some_and(|comma| start < comma)
            || !JOIN_WORDS
                .iter()
                .any(|word| sql[start..end].eq_ignore_ascii_case(word))
        {
            break;
        }
        join_start = Some(start);
    }
    join_start
        .map(Separator::Join)
        .or_else(|| last_comma.map(Separator::Comma))
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}
