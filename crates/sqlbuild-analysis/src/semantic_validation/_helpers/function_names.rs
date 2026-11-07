//! Built-in function spellings checked against the spellings the target dialect accepts.

use polyglot_sql::{DialectType, ValidationError};
use rustc_hash::{FxHashMap, FxHashSet};
use sqlbuild_sqltext::sql_scan::main::comment_end::comment_end;
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::models::{QuotePolicy, Unclosed};
use std::sync::LazyLock;

const UNKNOWN_FUNCTION: &str = "B101";
const FUNCTION_SPELLINGS: &str = include_str!("function_spellings.txt");
const SNOWFLAKE_DIALECT: &str = "snowflake";
const DUCKDB_DIALECT: &str = "duckdb";
const TYPE_POSITION_KEYWORD: &str = "AS";

/// Words after which an expression, and so a function call, may start.
const EXPRESSION_KEYWORDS: &str = "ALL AND ANY BETWEEN BY CASE DISTINCT ELSE ELSEIF ESCAPE HAVING \
    ILIKE INTERVAL IS LATERAL LIKE LIMIT MEASURES NOT OFFSET ON OR PRIOR QUALIFY REGEXP RETURN \
    RLIKE SELECT SET SOME THEN TOP WHEN WHERE WITH";
/// Clause keywords that end a `FROM` list or a `DEFINE` list at the same nesting depth.
const CLAUSE_KEYWORD_LIST: &str = "CONNECT EXCEPT FETCH GROUP HAVING INTERSECT LIMIT \
    MATCH_RECOGNIZE MEASURES MINUS OFFSET ON ORDER PATTERN PIVOT QUALIFY SAMPLE START TABLESAMPLE \
    UNION UNPIVOT USING WHERE WINDOW";
const FROM_KEYWORD: &str = "FROM";
const NOT_KEYWORD: &str = "NOT";
const MATERIALIZED_KEYWORD: &str = "MATERIALIZED";
const DISTINCT_KEYWORD: &str = "DISTINCT";

/// Where one rejected spelling applies: the dialect, the argument count and the accepted spelling.
type SpellingRow = (&'static str, usize, &'static str);

static EXPRESSION_STARTS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| EXPRESSION_KEYWORDS.split_whitespace().collect());
static CLAUSE_KEYWORDS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| CLAUSE_KEYWORD_LIST.split_whitespace().collect());
static ROOT_FRAME: Frame = Frame {
    select_seen: false,
    clause: Clause::Other,
};
static SPELLINGS: LazyLock<FxHashMap<&'static str, Vec<SpellingRow>>> =
    LazyLock::new(|| spellings(FUNCTION_SPELLINGS));

/// One call site whose authored built-in spelling the dialect does not accept.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UnsupportedCall {
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) suggestion: String,
}

/// The clause a scan is inside at one parenthesis depth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Clause {
    #[default]
    Other,
    From,
    Define,
}

/// Lexical state at one parenthesis depth.
#[derive(Debug, Default)]
struct Frame {
    select_seen: bool,
    clause: Clause,
}

/// Lexical context of the previous token, used to tell calls from relation and column lists.
struct Scan {
    frames: Vec<Frame>,
    previous_word: Option<String>,
    word_before: Option<String>,
    previous_byte: Option<u8>,
    path_colon: bool,
}

/// Incremental byte-to-character position, so locating every call stays linear in the SQL.
pub(crate) struct Cursor {
    byte: usize,
    pub(crate) characters: usize,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl Cursor {
    pub(crate) fn new() -> Self {
        Self {
            byte: 0,
            characters: 0,
            line: 1,
            column: 1,
        }
    }

    pub(crate) fn advance(&mut self, sql: &str, byte: usize) {
        for character in sql[self.byte..byte].chars() {
            self.characters += 1;
            if character == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        self.byte = byte;
    }
}

/// Read `dialect SPELLING arguments ACCEPTED` rows; `#` starts a comment.
fn spellings(source: &'static str) -> FxHashMap<&'static str, Vec<SpellingRow>> {
    let mut rows: FxHashMap<&'static str, Vec<SpellingRow>> = FxHashMap::default();
    for line in source.lines().map(str::trim) {
        let mut fields = line.split_whitespace();
        if let (Some(dialect), Some(spelling), Some(arity), Some(accepted)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
            && !dialect.starts_with('#')
            && let Ok(arity) = arity.parse::<usize>()
        {
            rows.entry(spelling)
                .or_default()
                .push((dialect, arity, accepted));
        }
    }
    rows
}

/// Return the accepted spelling for `upper` called with `arity` arguments in `dialect`.
fn accepted_spelling(dialect: &str, upper: &str, arity: usize) -> Option<&'static str> {
    SPELLINGS
        .get(upper)?
        .iter()
        .find_map(|(row_dialect, row_arity, accepted)| {
            (*row_dialect == dialect && *row_arity == arity).then_some(*accepted)
        })
}

/// The analysis dialect name the spelling table is keyed on, for the legacy validation path.
fn dialect_name(dialect: DialectType) -> Option<&'static str> {
    match dialect {
        DialectType::Snowflake => Some(SNOWFLAKE_DIALECT),
        DialectType::DuckDB => Some(DUCKDB_DIALECT),
        _ => None,
    }
}

/// Return located B101 diagnostics, with character spans, for unsupported built-in spellings.
pub(crate) fn unsupported_function_errors(sql: &str, dialect: DialectType) -> Vec<ValidationError> {
    let mut cursor: Cursor = Cursor::new();
    let Some(name) = dialect_name(dialect) else {
        return Vec::new();
    };
    unsupported_calls(sql, name)
        .into_iter()
        .map(|call| {
            let message: String = format!(
                "Unknown function '{}' for dialect {dialect:?}; did you mean {}?",
                call.name, call.suggestion
            );
            cursor.advance(sql, call.start);
            let start: usize = cursor.characters;
            let end: usize = start + sql[call.start..call.end].chars().count();
            ValidationError::error(message, UNKNOWN_FUNCTION)
                .with_location(cursor.line, cursor.column)
                .with_span(Some(start), Some(end))
        })
        .collect()
}

/// Return call sites in `sql` whose built-in spelling the analysis `dialect` does not accept.
pub(crate) fn unsupported_calls(sql: &str, dialect: &str) -> Vec<UnsupportedCall> {
    let bytes: &[u8] = sql.as_bytes();
    let mut calls: Vec<UnsupportedCall> = Vec::new();
    let mut scan: Scan = Scan::new();
    let mut index: usize = 0;
    while index < bytes.len() {
        match comment_end(bytes, index) {
            Ok(Some(end)) => {
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => break,
        }
        match non_code_end(bytes, index, QuotePolicy::COMPILER) {
            Ok(Some(end)) => {
                scan.punctuation(bytes[index]);
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => break,
        }
        let byte: u8 = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if !(byte.is_ascii_alphabetic() || byte == b'_') {
            scan.punctuation(byte);
            index += 1;
            continue;
        }
        let start: usize = index;
        while index < bytes.len() && is_word_byte(bytes[index]) {
            index += 1;
        }
        let name: &str = &sql[start..index];
        let upper: String = name.to_ascii_uppercase();
        let open: usize = skip_trivia(bytes, index);
        if bytes.get(open) == Some(&b'(')
            && scan.call_position()
            && let Some((arity, close)) = arguments(bytes, open)
            && !opens_cte_body(bytes, close + 1)
            && let Some(suggestion) = accepted_spelling(dialect, &upper, arity)
        {
            calls.push(UnsupportedCall {
                name: name.to_owned(),
                start,
                end: index,
                suggestion: suggestion.to_owned(),
            });
        }
        scan.word(upper);
    }
    calls
}

impl Scan {
    fn new() -> Self {
        Self {
            frames: vec![Frame::default()],
            previous_word: None,
            word_before: None,
            previous_byte: None,
            path_colon: false,
        }
    }

    fn frame(&self) -> &Frame {
        self.frames.last().unwrap_or(&ROOT_FRAME)
    }

    /// Return whether a name followed by `(` here is a call rather than a relation or column list.
    fn call_position(&self) -> bool {
        match self.previous_byte {
            Some(b'.' | b'@' | b')' | b'"' | b'`') => return false,
            Some(b':') if self.path_colon => return false,
            Some(b',') if self.frame().clause == Clause::From => return false,
            _ => {}
        }
        let Some(word) = self.previous_word.as_deref() else {
            return true;
        };
        match word {
            TYPE_POSITION_KEYWORD => self.frame().clause == Clause::Define,
            FROM_KEYWORD => !self.relation_from(),
            _ => EXPRESSION_STARTS.contains(word),
        }
    }

    /// Return whether a `FROM` just seen introduces relations rather than an operand.
    fn relation_from(&self) -> bool {
        self.frame().select_seen && self.word_before.as_deref() != Some(DISTINCT_KEYWORD)
    }

    fn punctuation(&mut self, byte: u8) {
        self.path_colon = byte == b':' && self.previous_byte != Some(b'\'');
        match byte {
            b'(' => self.frames.push(Frame::default()),
            b')' if self.frames.len() > 1 => {
                self.frames.pop();
            }
            _ => {}
        }
        self.previous_word = None;
        self.word_before = None;
        self.previous_byte = Some(byte);
    }

    fn word(&mut self, upper: String) {
        let relation_from: bool = upper == FROM_KEYWORD
            && self.frame().select_seen
            && self.previous_word.as_deref() != Some(DISTINCT_KEYWORD);
        if let Some(frame) = self.frames.last_mut() {
            match upper.as_str() {
                "SELECT" => {
                    frame.select_seen = true;
                    frame.clause = Clause::Other;
                }
                "JOIN" => frame.clause = Clause::From,
                FROM_KEYWORD if relation_from => frame.clause = Clause::From,
                "DEFINE" => frame.clause = Clause::Define,
                keyword if CLAUSE_KEYWORDS.contains(keyword) => frame.clause = Clause::Other,
                _ => {}
            }
        }
        self.word_before = self.previous_word.replace(upper);
        self.previous_byte = None;
        self.path_colon = false;
    }
}

/// Skip whitespace and comments between a name and its opening parenthesis.
fn skip_trivia(sql: &[u8], mut index: usize) -> usize {
    loop {
        while sql.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        match comment_end(sql, index) {
            Ok(Some(end)) => index = end,
            Ok(None) | Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => {
                return index;
            }
        }
    }
}

/// Return whether `AS [[NOT] MATERIALIZED] (` follows, so the parenthesis was a CTE column list.
fn opens_cte_body(sql: &[u8], index: usize) -> bool {
    let Some(mut index) = keyword_end(sql, index, TYPE_POSITION_KEYWORD) else {
        return false;
    };
    if let Some(end) = keyword_end(sql, index, NOT_KEYWORD) {
        let Some(end) = keyword_end(sql, end, MATERIALIZED_KEYWORD) else {
            return false;
        };
        index = end;
    } else if let Some(end) = keyword_end(sql, index, MATERIALIZED_KEYWORD) {
        index = end;
    }
    sql.get(skip_trivia(sql, index)) == Some(&b'(')
}

/// Return the end of `keyword` when it is the next token after trivia at `index`.
fn keyword_end(sql: &[u8], index: usize, keyword: &str) -> Option<usize> {
    let start: usize = skip_trivia(sql, index);
    let end: usize = start + keyword.len();
    (sql.get(start..end)?
        .eq_ignore_ascii_case(keyword.as_bytes())
        && !sql.get(end).copied().is_some_and(is_word_byte))
    .then_some(end)
}

pub(crate) fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Return the top-level argument count and closing index of the call opening at `open`.
fn arguments(sql: &[u8], open: usize) -> Option<(usize, usize)> {
    let mut depth: usize = 0;
    let mut commas: usize = 0;
    let mut empty: bool = true;
    let mut index: usize = open;
    while index < sql.len() {
        match comment_end(sql, index) {
            Ok(Some(end)) => {
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => return None,
        }
        match non_code_end(sql, index, QuotePolicy::COMPILER) {
            Ok(Some(end)) => {
                empty = false;
                index = end;
                continue;
            }
            Ok(None) => {}
            Err(Unclosed::BlockComment | Unclosed::Quote | Unclosed::Parenthesis) => return None,
        }
        match sql[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some((if empty { 0 } else { commas + 1 }, index));
                }
            }
            b',' if depth == 1 => commas += 1,
            byte if !byte.is_ascii_whitespace() => empty = false,
            _ => {}
        }
        index += 1;
    }
    None
}
