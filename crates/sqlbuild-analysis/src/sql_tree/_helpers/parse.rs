//! Parse SQL into statements with node spans.

use sqlquack_syntax::{CallMode, Expression, ParseOptions};

use crate::sql_tree::constants::GUARD_CODE_PREFIX;
use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::models::TreeParser;

/// The statements of `sql` under `parser`; a non-syntax PostgreSQL failure or a skipped statement is unanalysable.
pub(crate) fn parse(sql: &str, parser: TreeParser) -> Result<Vec<Expression>, SqlTreeError> {
    match parser {
        TreeParser::Shared(dialect) => sqlquack_syntax::parse_statements_with(
            sql,
            dialect,
            &ParseOptions::default(),
            CallMode::FaithfulLocated,
        )
        .map_err(|error| shared_error(&error.to_string())),
        TreeParser::Postgres => {
            let parsed = sqlquack_syntax_postgres::parse_strict(sql).map_err(|failure| {
                match failure.parse_error() {
                    Some(error) if error.syntax => SqlTreeError::Parse(error.message.clone()),
                    Some(error) => SqlTreeError::Unanalysable(error.message.clone()),
                    None => SqlTreeError::Parse("SQL has text outside any token".to_owned()),
                }
            })?;
            if !parsed.gaps.is_empty() {
                return Err(SqlTreeError::Unanalysable(
                    "a statement uses syntax the PostgreSQL parser skips".to_owned(),
                ));
            }
            Ok(parsed.statements)
        }
    }
}

/// A shared-parser failure: a guard limit, such as nesting depth, is unanalysable, not a syntax error.
fn shared_error(message: &str) -> SqlTreeError {
    if message.contains(GUARD_CODE_PREFIX) {
        SqlTreeError::Unanalysable(message.to_owned())
    } else {
        SqlTreeError::Parse(message.to_owned())
    }
}
