//! Snowflake's native module: SQLQuack's Snowflake parser reads its SQL. Validation only.

use sqlquack_semantics::dialects::constants::{SNOWFLAKE, SNOWFLAKE_QUOTED_IGNORING_CASE};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{
    Dialect, Error, Expression, ParseOptions, Parsed, StrictFailure, TokenizeFailure, parse_strict,
};

use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::constants::{
    SNOWFLAKE_DIALECT, SNOWFLAKE_TYPE_SPELLING, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{AnalysisInput, ParserGap, SnowflakeDialect};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for SnowflakeDialect {
    fn name(&self) -> &'static str {
        SNOWFLAKE_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        shared_parsed(sql, Dialect::Snowflake)
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &SNOWFLAKE
    }

    /// Snowflake's profile, with quoted names folded as well under QUOTED_IDENTIFIERS_IGNORE_CASE.
    fn profile_for(&self, input: &AnalysisInput<'_>) -> &'static dyn DialectProfile {
        match input.quoted_ignore_case {
            true => &SNOWFLAKE_QUOTED_IGNORING_CASE,
            false => &SNOWFLAKE,
        }
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
