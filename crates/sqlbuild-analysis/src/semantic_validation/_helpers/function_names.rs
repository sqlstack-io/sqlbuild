//! Built-in function spellings checked against the spellings the target dialect accepts.

use crate::semantic_validation::models::FunctionProbes;
use crate::semantic_validation::types::ProbeKey;
use polyglot_sql::{Dialect, DialectType, Expression, ValidationError};
use polyglot_sql_function_catalogs::{CatalogSink, FunctionNameCase, FunctionSignature};
use rustc_hash::{FxHashMap, FxHashSet};
use sqlbuild_sqltext::sql_scan::main::comment_end::comment_end;
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::models::{QuotePolicy, Unclosed};
use std::mem::Discriminant;
use std::sync::{LazyLock, OnceLock, PoisonError};

const UNKNOWN_FUNCTION: &str = "B101";
const SNOWFLAKE_FUNCTIONS: &str = include_str!("snowflake_functions.txt");
const DUCKDB_CATALOG: &str = "duckdb";
const PROBE_PREFIX: &str = "SELECT ";
const TYPE_POSITION_KEYWORD: &str = "AS";
const PROBE_ARITIES: usize = 9;
/// Keywords that may precede a parenthesis without being a function call.
const NON_CALL_KEYWORDS: &str = "ALL AND ANY AS ASOF AT BEFORE BETWEEN BY CASE CHANGES CONNECT CUBE \
    DEFINE DISTINCT DISTINCTROW ELSE END EXCEPT EXCLUDE EXISTS FILTER FROM HAVING ILIKE IN \
    INTERSECT INTERVAL IS JOIN LATERAL LIKE MATCH_CONDITION MATCH_RECOGNIZE MEASURES MINUS NOT ON \
    OR OVER PARTITION PATTERN PERCENT PIVOT PRIOR QUALIFY RANGE RENAME ROLLUP ROW ROWS SAMPLE \
    SELECT SETS SOME TABLE TABLESAMPLE THEN UNION UNPIVOT USING VALUES WHEN WHERE WITH WITHIN";

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
/// Syntax written like a call that no function catalogue lists: casts, EXTRACT and a CTE's
/// `[NOT] MATERIALIZED (` body.
const SYNTAX_CALLS: [&str; 5] = [
    "CAST",
    "TRY_CAST",
    "SAFE_CAST",
    "EXTRACT",
    MATERIALIZED_KEYWORD,
];
const DISTINCT_KEYWORD: &str = "DISTINCT";

type SpellingIndex = FxHashMap<Signature, Vec<String>>;

static KEYWORDS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| NON_CALL_KEYWORDS.split_whitespace().collect());
static EXPRESSION_STARTS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| EXPRESSION_KEYWORDS.split_whitespace().collect());
static CLAUSE_KEYWORDS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| CLAUSE_KEYWORD_LIST.split_whitespace().collect());
static ROOT_FRAME: Frame = Frame {
    select_seen: false,
    clause: Clause::Other,
};
static SNOWFLAKE: LazyLock<FxHashSet<String>> =
    LazyLock::new(|| accepted_names(SNOWFLAKE_FUNCTIONS));
static DUCKDB: LazyLock<FxHashSet<String>> = LazyLock::new(|| {
    let mut sink: CatalogNames = CatalogNames::default();
    polyglot_sql_function_catalogs::register_enabled_catalogs(&mut sink);
    sink.names
});
static SNOWFLAKE_INDEX: [OnceLock<SpellingIndex>; PROBE_ARITIES] =
    [const { OnceLock::new() }; PROBE_ARITIES];
static DUCKDB_INDEX: [OnceLock<SpellingIndex>; PROBE_ARITIES] =
    [const { OnceLock::new() }; PROBE_ARITIES];

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

/// How the parser represents one probe call.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Signature {
    /// A generic function node with this upper-case name.
    Named(String),
    /// A typed built-in node, identified by its expression variant.
    Typed(Discriminant<Expression>),
}

/// Upper-case DuckDB names from the SQL library's introspected function catalogue.
#[derive(Default)]
struct CatalogNames {
    names: FxHashSet<String>,
}

/// Incremental byte-to-character position, so locating every call stays linear in the SQL.
pub(crate) struct Cursor {
    byte: usize,
    pub(crate) characters: usize,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl CatalogSink for CatalogNames {
    fn set_dialect_name_case(&mut self, _dialect: &'static str, _name_case: FunctionNameCase) {}

    fn set_function_name_case(
        &mut self,
        _dialect: &'static str,
        _function_name: &str,
        _name_case: FunctionNameCase,
    ) {
    }

    fn register(
        &mut self,
        dialect: &'static str,
        function_name: &str,
        _signatures: Vec<FunctionSignature>,
    ) {
        if dialect == DUCKDB_CATALOG && is_identifier(function_name) {
            self.names.insert(function_name.to_ascii_uppercase());
        }
    }
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

impl FunctionProbes {
    /// Return the accepted spelling for an unaccepted call-site name the parser treats as built-in.
    fn suggestion(&self, dialect: DialectType, upper: &str, arity: usize) -> Option<String> {
        let key: ProbeKey = (dialect, upper.to_owned(), arity.min(PROBE_ARITIES - 1));
        if let Some(cached) = self
            .suggestions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return cached.clone();
        }
        let found: Option<String> = closest_spelling(dialect, upper, key.2);
        self.suggestions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, found.clone());
        found
    }
}

fn accepted_names(source: &str) -> FxHashSet<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_ascii_uppercase)
        .collect()
}

fn accepted(dialect: DialectType) -> Option<&'static FxHashSet<String>> {
    match dialect {
        DialectType::Snowflake => Some(&SNOWFLAKE),
        DialectType::DuckDB => Some(&DUCKDB),
        _ => None,
    }
}

/// Return located B101 diagnostics, with character spans, for unsupported built-in spellings.
pub(crate) fn unsupported_function_errors(
    sql: &str,
    dialect: DialectType,
    probes: &FunctionProbes,
) -> Vec<ValidationError> {
    let mut cursor: Cursor = Cursor::new();
    unsupported_calls(sql, dialect, probes)
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

/// Return call sites in `sql` whose built-in spelling `dialect` does not accept.
pub(crate) fn unsupported_calls(
    sql: &str,
    dialect: DialectType,
    probes: &FunctionProbes,
) -> Vec<UnsupportedCall> {
    let mut calls: Vec<UnsupportedCall> = Vec::new();
    unaccepted_calls(sql, dialect, |site| {
        if let Some(suggestion) = probes.suggestion(dialect, site.upper, site.arity) {
            calls.push(UnsupportedCall {
                name: site.name.to_owned(),
                start: site.start,
                end: site.end,
                suggestion,
            });
        }
    });
    calls
}

/// The distinct names, as written, of calls in `sql` that are neither built-ins of `dialect`,
/// nor one of its spellings SQLBuild suggests a fix for, nor `declared` by the project.
pub(crate) fn unknown_calls(
    sql: &str,
    dialect: DialectType,
    (probes, declared): (&FunctionProbes, &[String]),
) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    unaccepted_calls(sql, dialect, |site| {
        let known: bool = SYNTAX_CALLS.contains(&site.upper)
            || declared
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(site.name))
            || names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(site.name));
        if !known && probes.suggestion(dialect, site.upper, site.arity).is_none() {
            names.push(site.name.to_owned());
        }
    });
    names
}

/// One call site whose name the dialect's built-ins do not list.
struct CallSite<'s> {
    name: &'s str,
    upper: &'s str,
    start: usize,
    end: usize,
    arity: usize,
}

/// Each unqualified call in `sql` whose name `dialect` does not list as a built-in.
fn unaccepted_calls(sql: &str, dialect: DialectType, mut visit: impl FnMut(CallSite<'_>)) {
    let Some(names) = accepted(dialect) else {
        return;
    };
    let bytes: &[u8] = sql.as_bytes();
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
            && !name.starts_with("__")
            && !names.contains(&upper)
            && !KEYWORDS.contains(upper.as_str())
            && let Some((arity, close)) = arguments(bytes, open)
            && !opens_cte_body(bytes, close + 1)
        {
            visit(CallSite {
                name,
                upper: &upper,
                start,
                end: index,
                arity,
            });
        }
        scan.word(upper);
    }
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

fn is_identifier(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && name.bytes().all(is_word_byte)
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

/// Return the closest accepted spelling the parser maps to the same built-in node as `upper`.
fn closest_spelling(dialect: DialectType, upper: &str, arity: usize) -> Option<String> {
    let signature: Signature = parsed_signature(dialect, upper, arity)?;
    if signature == Signature::Named(upper.to_owned()) {
        return None;
    }
    spelling_index(dialect, arity)?
        .get(&signature)?
        .iter()
        .min_by_key(|accepted| (edit_distance(upper, accepted), (*accepted).clone()))
        .cloned()
}

fn spelling_index(dialect: DialectType, arity: usize) -> Option<&'static SpellingIndex> {
    let (names, indexes) = match dialect {
        DialectType::Snowflake => (&*SNOWFLAKE, &SNOWFLAKE_INDEX),
        DialectType::DuckDB => (&*DUCKDB, &DUCKDB_INDEX),
        _ => return None,
    };
    Some(indexes.get(arity)?.get_or_init(|| {
        let mut index: SpellingIndex = FxHashMap::default();
        for name in names {
            if let Some(signature) = parsed_signature(dialect, name, arity) {
                index.entry(signature).or_default().push(name.clone());
            }
        }
        index
    }))
}

fn parsed_signature(dialect: DialectType, upper: &str, arity: usize) -> Option<Signature> {
    let arguments: Vec<String> = (1..=arity).map(|index| format!("a{index}")).collect();
    let sql: String = format!("{PROBE_PREFIX}{upper}({})", arguments.join(", "));
    let Ok(statements) = Dialect::get(dialect).parse(&sql) else {
        return None;
    };
    let [Expression::Select(select)] = statements.as_slice() else {
        return None;
    };
    let [expression] = select.expressions.as_slice() else {
        return None;
    };
    match expression {
        Expression::Function(function) => {
            Some(Signature::Named(function.name.to_ascii_uppercase()))
        }
        Expression::AggregateFunction(function) => {
            Some(Signature::Named(function.name.to_ascii_uppercase()))
        }
        Expression::Column(_)
        | Expression::Identifier(_)
        | Expression::Tuple(_)
        | Expression::Paren(_) => None,
        other => Some(Signature::Typed(std::mem::discriminant(other))),
    }
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current: Vec<usize> = vec![row + 1; right.len() + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution: usize = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        previous = current;
    }
    previous[right.len()]
}
