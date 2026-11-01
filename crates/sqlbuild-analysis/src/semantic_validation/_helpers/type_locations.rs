//! Authored locations for native cast and set-operation findings that carry no span.

use crate::constants::DATETIME_TYPE;
use polyglot_sql::tokens::{Token, TokenType};
use sqlbuild_core::constants::TIMESTAMP_TYPE;

use polyglot_sql::{Dialect, DialectType, ValidationError};
use regex::Regex;
use sqlclean::sql_tokens::main::projection_spans::projection_spans;
use std::sync::LazyLock;

const INVALID_CAST: &str = "E218";
const LOSSY_CAST: &str = "W213";
const SET_OPERATION_CODES: [&str; 3] = ["E215", "E216", "W214"];
static CANNOT_CAST: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"^Cannot cast (\w+) to (\w+)$").map_err(|error| error.to_string())
});
static UNKNOWN_CAST_TYPE: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"^Unknown cast target type '([^']+)'$").map_err(|error| error.to_string())
});
static SET_OPERATION_COLUMN: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"^Set-operation column ([1-9][0-9]*) ").map_err(|error| error.to_string())
});
const COMPARISON_LITERAL: &str = "Comparison contains an invalid coercible literal";
const IN_LIST_LITERAL: &str = "IN list contains an invalid coercible literal";
const TEMPORAL_LITERAL: &str = "Invalid temporal literal";
const INTERVAL_LITERAL: &str = "Invalid interval literal";
const COMPARISON_OPERATORS: [TokenType; 7] = [
    TokenType::Eq,
    TokenType::Neq,
    TokenType::Lt,
    TokenType::Gt,
    TokenType::Lte,
    TokenType::Gte,
    TokenType::NullsafeEq,
];
const SET_OPERATION_ARITY: &str = "Set-operation operands return different column counts";

struct CastSite {
    start: usize,
    end: usize,
    target: String,
    operand_words: Vec<String>,
    literal_operand: bool,
}

struct SetOperationSite {
    keyword: (usize, usize),
    projections: Vec<(usize, usize)>,
}

/// Point each position-less cast, literal and set-operation finding at its own expression.
pub(crate) fn locate_type_findings(
    sql: &str,
    dialect: DialectType,
    errors: &mut [ValidationError],
) -> Result<(), String> {
    if !errors.iter().any(|error| {
        error.start.is_none() && (is_cast(&error.code) || is_set_operation(&error.code))
    }) {
        return Ok(());
    }
    let Ok(tokens) = Dialect::get(dialect).tokenize(sql) else {
        return Ok(());
    };
    let positioned: Vec<usize> = errors
        .iter()
        .filter(|error| is_cast(&error.code))
        .filter_map(|error| error.start)
        .collect();
    let mut locator = Locator::new(&tokens, &positioned);
    for error in errors.iter_mut().filter(|error| error.start.is_none()) {
        if let Some((start, end)) = locator.span(&error.code, &error.message)? {
            error.start = Some(start);
            error.end = Some(end);
        }
    }
    Ok(())
}

struct Locator {
    casts: Vec<CastSite>,
    used_casts: Vec<bool>,
    set_operations: Vec<SetOperationSite>,
    used_columns: Vec<Vec<bool>>,
    used_arity: Vec<bool>,
    literals: LiteralSites,
}

impl Locator {
    fn new(tokens: &[Token], positioned: &[usize]) -> Self {
        let casts: Vec<CastSite> = cast_sites(tokens);
        let used_casts: Vec<bool> = casts
            .iter()
            .map(|site| contains_any(site, positioned))
            .collect();
        let set_operations: Vec<SetOperationSite> = set_operation_sites(tokens);
        let used_columns: Vec<Vec<bool>> = set_operations
            .iter()
            .map(|site| vec![false; site.projections.len()])
            .collect();
        let used_arity: Vec<bool> = vec![false; set_operations.len()];
        Self {
            casts,
            used_casts,
            set_operations,
            used_columns,
            used_arity,
            literals: LiteralSites::new(tokens),
        }
    }

    fn span(&mut self, code: &str, message: &str) -> Result<Option<(usize, usize)>, String> {
        if is_cast(code) {
            if let Some(span) = self.literals.next(message) {
                return Ok(Some(span));
            }
            return self.cast_span(message);
        }
        if is_set_operation(code) {
            return self.set_operation_span(message);
        }
        Ok(None)
    }

    fn cast_span(&mut self, message: &str) -> Result<Option<(usize, usize)>, String> {
        let Some(preference) = CastPreference::from_message(message)? else {
            return Ok(None);
        };
        let index: Option<usize> = (0..self.casts.len())
            .filter(|&index| !self.used_casts[index])
            .min_by_key(|&index| (preference.rank(&self.casts[index]), index));
        Ok(index.map(|index| {
            self.used_casts[index] = true;
            (self.casts[index].start, self.casts[index].end)
        }))
    }

    fn set_operation_span(&mut self, message: &str) -> Result<Option<(usize, usize)>, String> {
        if message.starts_with(SET_OPERATION_ARITY) {
            let index: Option<usize> =
                (0..self.set_operations.len()).find(|&index| !self.used_arity[index]);
            return Ok(index.map(|index| {
                self.used_arity[index] = true;
                self.set_operations[index].keyword
            }));
        }
        let Some(captures) = SET_OPERATION_COLUMN
            .as_ref()
            .map_err(Clone::clone)?
            .captures(message)
        else {
            return Ok(None);
        };
        let Ok(ordinal) = captures[1].parse::<usize>() else {
            return Ok(None);
        };
        let column: usize = ordinal - 1;
        let index: Option<usize> = (0..self.set_operations.len()).find(|&index| {
            self.set_operations[index].projections.len() > column
                && !self.used_columns[index][column]
        });
        Ok(index.map(|index| {
            self.used_columns[index][column] = true;
            self.set_operations[index].projections[column]
        }))
    }
}

fn contains_any(site: &CastSite, starts: &[usize]) -> bool {
    starts
        .iter()
        .any(|&start| site.start <= start && start < site.end)
}

/// How well a cast site explains one cast finding; lower ranks are better matches.
enum CastPreference {
    Conversion { source: String, target: String },
    UnknownTarget(String),
}

impl CastPreference {
    fn from_message(message: &str) -> Result<Option<Self>, String> {
        if let Some(captures) = CANNOT_CAST
            .as_ref()
            .map_err(Clone::clone)?
            .captures(message)
        {
            return Ok(Some(Self::Conversion {
                source: captures[1].to_ascii_lowercase(),
                target: captures[2].to_ascii_lowercase(),
            }));
        }
        Ok(UNKNOWN_CAST_TYPE
            .as_ref()
            .map_err(Clone::clone)?
            .captures(message)
            .map(|captures| Self::UnknownTarget(captures[1].to_ascii_lowercase())))
    }

    fn rank(&self, site: &CastSite) -> u8 {
        match self {
            Self::Conversion { source, target } => {
                let target_matches: bool = type_family(&site.target) == target;
                let source_matches: bool = site.operand_words.contains(source);
                match (target_matches, source_matches, site.literal_operand) {
                    (true, true, true) => 0,
                    (true, true, false) => 1,
                    (true, false, true) => 2,
                    _ => 3,
                }
            }
            Self::UnknownTarget(name) if site.target.to_ascii_lowercase() == *name => 0,
            Self::UnknownTarget(_) => 3,
        }
    }
}

/// Literal sites for coercion findings, consumed in source order per finding kind.
struct LiteralSites {
    comparisons: Vec<(usize, usize)>,
    in_lists: Vec<(usize, usize)>,
    temporals: Vec<(usize, usize)>,
    intervals: Vec<(usize, usize)>,
}

impl LiteralSites {
    fn new(tokens: &[Token]) -> Self {
        let mut sites: Self = Self {
            comparisons: Vec::new(),
            in_lists: Vec::new(),
            temporals: Vec::new(),
            intervals: Vec::new(),
        };
        for (index, token) in tokens.iter().enumerate() {
            let previous: Option<&Token> = index.checked_sub(1).map(|previous| &tokens[previous]);
            let next: Option<&Token> = tokens.get(index + 1);
            let through_next: (usize, usize) = (
                token.span.start,
                next.map_or(token.span.end, |next| next.span.end),
            );
            match token.token_type {
                TokenType::String if is_comparison_operand(previous, next) => {
                    sites.comparisons.push((token.span.start, token.span.end));
                }
                TokenType::In if next.is_some_and(|next| next.token_type == TokenType::LParen) => {
                    sites.in_lists.push((token.span.start, token.span.end));
                }
                TokenType::Date | TokenType::Timestamp | TokenType::DateTime
                    if next.is_some_and(|next| next.token_type == TokenType::String) =>
                {
                    sites.temporals.push(through_next);
                }
                TokenType::Interval => sites.intervals.push(through_next),
                _ => {}
            }
        }
        sites.comparisons.reverse();
        sites.in_lists.reverse();
        sites.temporals.reverse();
        sites.intervals.reverse();
        sites
    }

    fn next(&mut self, message: &str) -> Option<(usize, usize)> {
        match message {
            COMPARISON_LITERAL => self.comparisons.pop(),
            IN_LIST_LITERAL => self.in_lists.pop(),
            TEMPORAL_LITERAL => self.temporals.pop(),
            INTERVAL_LITERAL => self.intervals.pop(),
            _ => None,
        }
    }
}

fn is_comparison_operand(previous: Option<&Token>, next: Option<&Token>) -> bool {
    previous
        .into_iter()
        .chain(next)
        .any(|neighbour| COMPARISON_OPERATORS.contains(&neighbour.token_type))
}

fn is_cast(code: &str) -> bool {
    code == INVALID_CAST || code == LOSSY_CAST
}

fn is_set_operation(code: &str) -> bool {
    SET_OPERATION_CODES.contains(&code)
}

fn cast_sites(tokens: &[Token]) -> Vec<CastSite> {
    let mut sites: Vec<CastSite> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.token_type {
            TokenType::Cast | TokenType::TryCast | TokenType::SafeCast
                if tokens
                    .get(index + 1)
                    .is_some_and(|next| next.token_type == TokenType::LParen) =>
            {
                let Some(close) = matching_paren(tokens, index + 1) else {
                    continue;
                };
                let mut depth = 0usize;
                let mut target_start = None;
                for (offset, inner) in tokens[index + 2..close].iter().enumerate() {
                    match inner.token_type {
                        TokenType::LParen => depth += 1,
                        TokenType::RParen => depth = depth.saturating_sub(1),
                        TokenType::As if depth == 0 => target_start = Some(index + 3 + offset),
                        _ => {}
                    }
                }
                if let Some(target_start) = target_start {
                    let operand = &tokens[index + 2..target_start - 1];
                    sites.push(CastSite {
                        start: token.span.start,
                        end: tokens[close].span.end,
                        target: type_text(&tokens[target_start..close]),
                        operand_words: operand_words(operand),
                        literal_operand: literal_operand(operand),
                    });
                }
            }
            TokenType::DColon => {
                let Some(first) = tokens.get(index + 1) else {
                    continue;
                };
                let mut last = index + 1;
                if tokens
                    .get(index + 2)
                    .is_some_and(|next| next.token_type == TokenType::LParen)
                    && let Some(close) = matching_paren(tokens, index + 2)
                {
                    last = close;
                }
                let operand = &tokens[index.saturating_sub(1)..index];
                sites.push(CastSite {
                    start: index
                        .checked_sub(1)
                        .map_or(token.span.start, |previous| tokens[previous].span.start),
                    end: tokens[last].span.end,
                    target: first.text.clone(),
                    operand_words: operand_words(operand),
                    literal_operand: literal_operand(operand),
                });
            }
            _ => {}
        }
    }
    sites.sort_by_key(|site| site.start);
    sites
}

fn set_operation_sites(tokens: &[Token]) -> Vec<SetOperationSite> {
    let mut sites: Vec<SetOperationSite> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(
            token.token_type,
            TokenType::Union | TokenType::Except | TokenType::Intersect
        ) {
            continue;
        }
        let Some(select) = tokens[index + 1..]
            .iter()
            .position(|next| next.token_type == TokenType::Select)
            .map(|offset| index + 1 + offset)
        else {
            continue;
        };
        if tokens[index + 1..select].iter().any(|between| {
            !matches!(
                between.token_type,
                TokenType::All | TokenType::Distinct | TokenType::LParen
            ) && !between.text.eq_ignore_ascii_case("by")
                && !between.text.eq_ignore_ascii_case("name")
        }) {
            continue;
        }
        sites.push(SetOperationSite {
            keyword: (token.span.start, token.span.end),
            projections: projection_spans(tokens, select),
        });
    }
    sites
}

fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.token_type {
            TokenType::LParen => depth += 1,
            TokenType::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn operand_words(tokens: &[Token]) -> Vec<String> {
    tokens
        .iter()
        .map(|token| type_family(&token.text).to_owned())
        .chain(tokens.iter().map(|token| token.text.to_ascii_lowercase()))
        .collect()
}

fn literal_operand(tokens: &[Token]) -> bool {
    !tokens
        .iter()
        .any(|token| matches!(token.token_type, TokenType::Var | TokenType::Identifier))
}

fn type_text(tokens: &[Token]) -> String {
    tokens
        .first()
        .map(|token| token.text.clone())
        .unwrap_or_default()
}

fn type_family(type_name: &str) -> &'static str {
    match type_name.to_ascii_uppercase().as_str() {
        "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT" | "HUGEINT" | "UBIGINT"
        | "UINTEGER" | "USMALLINT" | "UTINYINT" | "INT2" | "INT4" | "INT8" | "BYTEINT" => "integer",
        "NUMBER" | "NUMERIC" | "DECIMAL" | "FLOAT" | "DOUBLE" | "REAL" | "FLOAT4" | "FLOAT8" => {
            "numeric"
        }
        "BOOLEAN" | "BOOL" => "boolean",
        name if name.starts_with(TIMESTAMP_TYPE) || name == DATETIME_TYPE => "timestamp",
        _ => "other",
    }
}
