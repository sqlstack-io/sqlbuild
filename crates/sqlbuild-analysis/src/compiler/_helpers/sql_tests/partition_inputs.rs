//! A mocked call's `TABLE( )` dropped, and a partitioned call's inputs read as one row, as a build gives them.

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
const SELECT_KEYWORD: &str = "select";
const TABLE_KEYWORD: &str = "table";
const AS_KEYWORD: &str = "as";
const PARTITION_KEYWORD: &str = "partition";
const BY_KEYWORD: &str = "by";
const CROSS_JOIN: &str = " CROSS JOIN ";
const INPUTS_ALIAS: &str = "__sqb_partition_inputs";
const CALL_ALIAS: &str = "__sqb_partition_call";
const JOIN_WORDS: [&str; 8] = [
    "join", "cross", "inner", "left", "right", "full", "outer", "natural",
];
const SET_OPERATORS: [&str; 4] = ["union", "intersect", "except", "minus"];
/// Words after a relation that are not its alias.
const CLAUSE_WORDS: [&str; 24] = [
    "where",
    "group",
    "having",
    "qualify",
    "order",
    "limit",
    "offset",
    "fetch",
    "window",
    "on",
    "using",
    "join",
    "cross",
    "inner",
    "left",
    "right",
    "full",
    "natural",
    "lateral",
    "union",
    "intersect",
    "except",
    "minus",
    "match_recognize",
];

/// The edits to one SQL text, by start: each one's end and replacement.
type Edits = BTreeMap<usize, (usize, String)>;

/// `sql` with each mocked call out of `TABLE( )` and each mocked partitioned call's inputs as `p RIGHT JOIN (SELECT 1) ON FALSE`, its `PARTITION BY` columns read from the fixture's columns of the same name.
pub(crate) fn collapse_partition_inputs(
    sql: &str,
    pattern: &Regex,
    syntax: &LexicalSyntax,
    fixture_columns: impl Fn(&str) -> Option<Vec<String>>,
) -> Result<String, String> {
    let mut protected = ProtectedRanges::new(syntax, sql);
    let mut edits: Edits = BTreeMap::new();
    let mut collapsed = 0;
    for captures in pattern.captures_iter(sql) {
        let (Some(full), Some(name)) = (captures.get(0), captures.get(1)) else {
            continue;
        };
        if protected.contains(full.start()) {
            continue;
        }
        let Some(columns) = fixture_columns(name.as_str()) else {
            continue;
        };
        let arguments = skip_whitespace(sql, full.end());
        if !sql[arguments..].starts_with('(') {
            continue;
        }
        let Ok(close) = dialect_matching_paren(sql.as_bytes(), arguments, syntax) else {
            continue;
        };
        let over = over_clause_open(sql, close + 1, &syntax.line_comment_prefixes)
            .filter(|over| !protected.contains(*over));
        let over_close = match over.map(|over| dialect_matching_paren(sql.as_bytes(), over, syntax))
        {
            Some(Ok(over_close)) => Some(over_close),
            Some(Err(_)) => continue,
            None => None,
        };
        let call = (full.start(), over_close.unwrap_or(close) + 1);
        let (relation, unwrapping) = unwrapped(sql, call, syntax);
        edits.extend(unwrapping);
        let (Some(over), Some(over_close)) = (over, over_close) else {
            continue;
        };
        let call_start = lateral_start(sql, relation.0, |index| !protected.contains(index))
            .unwrap_or(relation.0);
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
        for (start, edit) in collapse(sql, (&scan.commas, separator), collapsed) {
            let _ = edits.entry(start).or_insert(edit);
        }
        let references = partition_references(sql, (over, over_close), &columns);
        if !references.is_empty() {
            let scope = select_scope(sql, (scan.from, relation.1), &mut protected);
            let read = PartitionRead {
                relation,
                references: &references,
                scope,
                ordinal: collapsed,
            };
            for (start, edit) in read_partition_columns(sql, &read, &mut protected) {
                let _ = edits.entry(start).or_insert(edit);
            }
        }
        collapsed += 1;
    }
    let mut output = String::with_capacity(sql.len());
    let mut cursor = 0;
    for (start, (end, text)) in edits {
        if start < cursor {
            continue;
        }
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
    /// Where the clause's `FROM` keyword starts.
    from: usize,
    /// The top-level commas between `FROM` and the call.
    commas: Vec<usize>,
    /// What joins the call to them, or `None` when the call follows `FROM` directly.
    separator: Option<Separator>,
}

/// A partitioned call whose `PARTITION BY` columns read the fixture's columns.
struct PartitionRead<'a> {
    /// The call as a relation, `TABLE( )` included.
    relation: (usize, usize),
    /// Each `qualifier.column` partition reference whose column the fixture has.
    references: &'a [(String, String)],
    /// The SELECT the call stands in.
    scope: (usize, usize),
    ordinal: usize,
}

/// The call's extent as a relation, and the edits that remove an enclosing `TABLE( )`.
fn unwrapped(
    sql: &str,
    (start, end): (usize, usize),
    syntax: &LexicalSyntax,
) -> ((usize, usize), Edits) {
    let before = sql[..start].trim_end();
    if !before.ends_with('(') {
        return ((start, end), Edits::new());
    }
    let open = before.len() - 1;
    let word = sql[..open].trim_end();
    let Some(table) = word.len().checked_sub(TABLE_KEYWORD.len()) else {
        return ((start, end), Edits::new());
    };
    let close = skip_whitespace(sql, end);
    let wrapped = word[table..].eq_ignore_ascii_case(TABLE_KEYWORD)
        && !word[..table].bytes().next_back().is_some_and(is_word_byte)
        && matches!(dialect_matching_paren(sql.as_bytes(), open, syntax), Ok(found) if found == close);
    if !wrapped {
        return ((start, end), Edits::new());
    }
    let edits: Edits = [
        (table, (start, String::new())),
        (end, (close + 1, String::new())),
    ]
    .into_iter()
    .collect();
    ((table, close + 1), edits)
}

/// The edits that join the inputs before the call as one row of NULLs.
fn collapse(sql: &str, (commas, separator): (&[usize], Separator), ordinal: usize) -> Edits {
    let mut edits: Edits = Edits::new();
    for &comma in commas {
        let _ = edits.insert(
            comma,
            (skip_whitespace(sql, comma + 1), CROSS_JOIN.to_owned()),
        );
    }
    let collapse =
        format!(" RIGHT JOIN (SELECT 1 AS {INPUTS_ALIAS}) AS {INPUTS_ALIAS}_{ordinal} ON FALSE");
    match separator {
        Separator::Comma(comma) => {
            let end = skip_whitespace(sql, comma + 1);
            let _ = edits.insert(comma, (end, format!("{collapse}{CROSS_JOIN}")));
        }
        Separator::Join(start) => {
            let _ = edits.insert(start, (start, format!("{} ", collapse.trim_start())));
        }
    }
    edits
}

/// The edits that point each partition reference in the call's SELECT at the call's row, naming the call when it has no alias.
fn read_partition_columns(
    sql: &str,
    read: &PartitionRead<'_>,
    protected: &mut ProtectedRanges<'_>,
) -> Edits {
    let mut edits: Edits = Edits::new();
    let alias = match call_alias(sql, read.relation.1) {
        Alias::Named(alias) => alias,
        Alias::Renamed => return edits,
        Alias::Missing => {
            let alias = format!("{CALL_ALIAS}_{}", read.ordinal);
            let _ = edits.insert(read.relation.1, (read.relation.1, format!(" AS {alias}")));
            alias
        }
    };
    let lower = sql.to_ascii_lowercase();
    for (qualifier, column) in read.references {
        let reference = format!("{qualifier}.{column}");
        let mut from = read.scope.0;
        while let Some(found) = lower[from..read.scope.1].find(&reference) {
            let start = from + found;
            let end = start + reference.len();
            from = end;
            let standalone = !sql[..start]
                .bytes()
                .next_back()
                .is_some_and(|byte| is_word_byte(byte) || byte == b'.')
                && !sql.as_bytes().get(end).copied().is_some_and(is_word_byte);
            let outside_call = start < read.relation.0 || start >= read.relation.1;
            if standalone && outside_call && !protected.contains(start) {
                let _ = edits
                    .entry(start)
                    .or_insert((end, format!("{alias}.{}", &sql[end - column.len()..end])));
            }
        }
    }
    edits
}

/// How the relation after `end` is named.
enum Alias {
    Named(String),
    /// Named with a column list, which renames the fixture's columns.
    Renamed,
    Missing,
}

/// The alias written after a relation ending at `end`.
fn call_alias(sql: &str, end: usize) -> Alias {
    let mut start = skip_whitespace(sql, end);
    let mut word = word_at(sql, start);
    if word.eq_ignore_ascii_case(AS_KEYWORD) {
        start = skip_whitespace(sql, start + word.len());
        word = word_at(sql, start);
    }
    let is_clause = CLAUSE_WORDS
        .iter()
        .any(|clause| word.eq_ignore_ascii_case(clause));
    if word.is_empty() || is_clause {
        return Alias::Missing;
    }
    if sql[skip_whitespace(sql, start + word.len())..].starts_with('(') {
        return Alias::Renamed;
    }
    Alias::Named(word.to_owned())
}

/// The word of identifier bytes starting at `start`.
fn word_at(sql: &str, start: usize) -> &str {
    let length = sql.as_bytes()[start..]
        .iter()
        .take_while(|byte| is_word_byte(**byte))
        .count();
    &sql[start..start + length]
}

/// Each `qualifier.column` expression of the `PARTITION BY` list in the `OVER (...)` at `(open, close)`, lowercased, whose column `columns` has.
fn partition_references(
    sql: &str,
    (open, close): (usize, usize),
    columns: &[String],
) -> Vec<(String, String)> {
    let clause = sql[open + 1..close].trim_start();
    let mut words = clause.split_ascii_whitespace();
    let partitioned = words
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case(PARTITION_KEYWORD))
        && words
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case(BY_KEYWORD));
    if !partitioned {
        return Vec::new();
    }
    let list = clause[PARTITION_KEYWORD.len()..].trim_start()[BY_KEYWORD.len()..]
        .to_ascii_lowercase()
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let list = list.split(" order by ").next().unwrap_or_default();
    let mut references: Vec<(String, String)> = Vec::new();
    for expression in list.split(',') {
        let Some((qualifier, column)) = expression.trim().split_once('.') else {
            continue;
        };
        let known = columns
            .iter()
            .any(|known| known.eq_ignore_ascii_case(column));
        if simple_name(qualifier) && simple_name(column) && known {
            references.push((qualifier.to_owned(), column.to_owned()));
        }
    }
    references
}

/// Whether `name` is one unquoted identifier.
fn simple_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_word_byte)
}

/// The SELECT whose FROM keyword starts at `from`: from its `SELECT` to the end of its query, after `after`.
fn select_scope(
    sql: &str,
    (from, after): (usize, usize),
    protected: &mut ProtectedRanges<'_>,
) -> (usize, usize) {
    let bytes = sql.as_bytes();
    let mut start = 0;
    let mut depth = 0usize;
    let mut index = from;
    while index > 0 {
        index -= 1;
        if protected.contains(index) {
            continue;
        }
        match bytes[index] {
            b')' => depth += 1,
            b'(' if depth == 0 => {
                start = index + 1;
                break;
            }
            b'(' => depth -= 1,
            byte if depth == 0 && is_word_byte(byte) => {
                let end = index + 1;
                while index > 0 && is_word_byte(bytes[index - 1]) {
                    index -= 1;
                }
                if sql[index..end].eq_ignore_ascii_case(SELECT_KEYWORD) {
                    start = index;
                    break;
                }
            }
            _ => {}
        }
    }
    let mut end = sql.len();
    let mut depth = 0usize;
    let mut index = after;
    while index < bytes.len() {
        if protected.contains(index) {
            index += 1;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' if depth == 0 => {
                end = index;
                break;
            }
            b')' => depth -= 1,
            byte if depth == 0 && is_word_byte(byte) => {
                let word = word_at(sql, index);
                if SET_OPERATORS
                    .iter()
                    .any(|operator| word.eq_ignore_ascii_case(operator))
                {
                    end = index;
                    break;
                }
                index += word.len();
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    (start, end)
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
                        from: index,
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
