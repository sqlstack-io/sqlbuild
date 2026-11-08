//! Snowflake's native module: SQLQuack's Snowflake parser decides whether a model is read whole.

use sqlquack_syntax::{
    Dialect, Error, Expression, ParseOptions, Parsed, StrictFailure, TokenizeFailure, parse_strict,
};

use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::_helpers::whole::one_statement;
use crate::analysis_route::constants::{
    SNOWFLAKE_DIALECT, SNOWFLAKE_TYPE_SPELLING, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{ParserGap, SnowflakeDialect};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for SnowflakeDialect {
    fn name(&self) -> &'static str {
        SNOWFLAKE_DIALECT
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let parsed: Parsed = self.parse(sql)?;
        one_statement(&parsed.statements, &parsed.gaps, sql.len())
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        shared_parsed(sql, Dialect::Snowflake)
    }

    /// The type as Polyglot normalised it, or `None` (unknown) where the text fallback can't match it.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        respelt(spelling, &SNOWFLAKE_TYPE_SPELLING)
            .map(|respelt| normalize_text_type(&respelt, Some(TypeDialect::Snowflake)))
    }
}

/// Every statement the shared parser reads from `sql` in `dialect`, or its first refusal; it
/// cannot tell invalid syntax from an unsupported construct, so no refusal is marked syntax.
pub(crate) fn shared_parsed(sql: &str, dialect: Dialect) -> Result<Parsed, ParserGap> {
    let statements: Vec<Expression> = parse_strict(sql, dialect, &ParseOptions::default())
        .map_err(|failure| match failure {
            StrictFailure::Tokens(
                TokenizeFailure::Uncovered { start, .. } | TokenizeFailure::Overlap { start, .. },
            ) => ParserGap {
                construct: UNSCANNED_CONSTRUCT.to_owned(),
                message: UNSCANNED_CONSTRUCT.to_owned(),
                offset: start,
                syntax: false,
            },
            StrictFailure::Parse(error)
            | StrictFailure::Tokens(TokenizeFailure::Tokenizer(error)) => {
                let offset: usize = error.start().unwrap_or_default();
                let message: String = unplaced(error);
                ParserGap {
                    construct: message.clone(),
                    message,
                    offset,
                    syntax: false,
                }
            }
        })?;
    Ok(Parsed {
        statements,
        gaps: Vec::new(),
    })
}

/// The parser's message without its own line and column, which P026 reports from the offset.
fn unplaced(error: Error) -> String {
    match error {
        Error::Tokenize { message, .. }
        | Error::Parse { message, .. }
        | Error::Syntax { message, .. } => message,
        other => other.to_string(),
    }
}
