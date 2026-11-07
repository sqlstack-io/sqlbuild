//! Snowflake's native module: SQLQuack's Snowflake parser decides whether a model is read whole.

use sqlquack_syntax::{
    Dialect, Error, Expression, ParseOptions, StrictFailure, TokenizeFailure, parse_strict,
};

use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::_helpers::whole::one_statement;
use crate::analysis_route::constants::{
    SNOWFLAKE_DIALECT, SNOWFLAKE_TYPE_SPELLING, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{ParserGap, SnowflakeDialect};
use crate::analysis_route::types::NativeDialect;

impl NativeDialect for SnowflakeDialect {
    fn name(&self) -> &'static str {
        SNOWFLAKE_DIALECT
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let statements: Vec<Expression> =
            parse_strict(sql, Dialect::Snowflake, &ParseOptions::default()).map_err(|failure| {
                match failure {
                    StrictFailure::Tokens(
                        TokenizeFailure::Uncovered { start, .. }
                        | TokenizeFailure::Overlap { start, .. },
                    ) => ParserGap {
                        construct: UNSCANNED_CONSTRUCT.to_owned(),
                        offset: start,
                    },
                    StrictFailure::Parse(error)
                    | StrictFailure::Tokens(TokenizeFailure::Tokenizer(error)) => ParserGap {
                        offset: error.start().unwrap_or_default(),
                        construct: unplaced(error),
                    },
                }
            })?;
        one_statement(&statements, &[], sql.len())
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }
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

impl SnowflakeDialect {
    /// A type spelt so the type fallback gives Polyglot's result, or `None` (unknown) where it can't.
    #[must_use]
    pub fn normalize_type(&self, spelling: &str) -> Option<String> {
        respelt(spelling, &SNOWFLAKE_TYPE_SPELLING)
    }
}
