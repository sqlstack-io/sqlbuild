//! Parse SQL into statements with node spans.

use sqlquack_syntax::{CallMode, Expression, ParseOptions};

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
        .map_err(|error| SqlTreeError::Parse(error.to_string())),
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
