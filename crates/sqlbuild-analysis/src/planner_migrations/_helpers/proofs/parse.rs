//! Parse migration SQL under the compiler's complexity limit.

use polyglot_sql::dialects::Dialect;

use serde_json::Value;

use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;

pub(crate) fn parse(sql: &str, dialect: &str) -> Result<Option<Value>, String> {
    let dialect =
        Dialect::get_by_name(dialect).ok_or_else(|| format!("unknown dialect {dialect}"))?;
    let Ok(mut statements) = dialect.parse_with_options(sql, &polyglot_parse_options()) else {
        return Ok(None);
    };
    if statements.len() != 1 {
        return Ok(None);
    }
    serde_json::to_value(statements.remove(0))
        .map(Some)
        .map_err(|error| error.to_string())
}
