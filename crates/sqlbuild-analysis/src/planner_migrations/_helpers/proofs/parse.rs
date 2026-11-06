//! Parse migration SQL under the compiler's complexity limit.

use crate::lineage::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::planner_migrations::models::ParsedStatement;
use polyglot_sql::dialects::Dialect;
use polyglot_sql::{ComplexityGuardOptions, ParseOptions};
use serde_json::{Value, json};

pub(crate) fn parse(sql: &str, dialect: &str) -> Result<Option<Value>, String> {
    let dialect =
        Dialect::get_by_name(dialect).ok_or_else(|| format!("unknown dialect {dialect}"))?;
    let guard: ComplexityGuardOptions =
        serde_json::from_value(json!({"maxFunctionCallDepth": MAX_FUNCTION_CALL_DEPTH}))
            .map_err(|error| error.to_string())?;
    let options = ParseOptions {
        complexity_guard: Some(guard),
    };
    let Ok(mut statements) = dialect.parse_with_options(sql, &options) else {
        return Ok(None);
    };
    if statements.len() != 1 {
        return Ok(None);
    }
    serde_json::to_value(statements.remove(0))
        .map(Some)
        .map_err(|error| error.to_string())
}

/// The statement tree with its token spans, or None when either step rejects the SQL.
pub(crate) fn parse_statement(sql: &str, dialect: &str) -> Result<Option<ParsedStatement>, String> {
    let Some(tree) = parse(sql, dialect)? else {
        return Ok(None);
    };
    let Ok(tokens) = Dialect::get_by_name(dialect)
        .ok_or_else(|| format!("unknown dialect {dialect}"))?
        .tokenize(sql)
    else {
        return Ok(None);
    };
    Ok(Some(ParsedStatement {
        tree,
        tokens: tokens
            .iter()
            .map(|token| token.span.start..token.span.end)
            .collect(),
    }))
}
