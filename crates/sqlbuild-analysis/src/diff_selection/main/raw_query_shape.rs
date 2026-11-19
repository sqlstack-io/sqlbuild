//! Parse one raw diff query as the Polyglot wheel's guarded `parse_one` does and read its shape.

use polyglot_sql::Expression;
use polyglot_sql::dialects::Dialect;
use polyglot_sql::traversal::ExpressionWalk;

use crate::diff_selection::constants::RAW_QUERY_READ_ONLY_ROOTS;
use crate::diff_selection::models::RawQueryShape;
use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;

/// The query's shape, or the text of the exception the wheel raises for it.
pub fn raw_query_shape(sql: &str, dialect: &str) -> Result<RawQueryShape, String> {
    let parser: Dialect =
        Dialect::get_by_name(dialect).ok_or_else(|| format!("Unknown dialect: {dialect}"))?;
    let mut statements: Vec<Expression> = parser
        .parse_with_options(sql, &polyglot_parse_options())
        .map_err(|error| error.to_string())?;
    if statements.len() != 1 {
        return Err(format!("Expected 1 statement, found {}", statements.len()));
    }
    let root: Expression = statements.remove(0);
    let root_key: String = root.variant_name().to_lowercase();
    let node_keys: Vec<String> = if RAW_QUERY_READ_ONLY_ROOTS.contains(&root_key.as_str()) {
        root.dfs()
            .map(|node| node.variant_name().to_lowercase())
            .collect()
    } else {
        Vec::new()
    };
    let writes_or_locks: bool = match &root {
        Expression::Select(select) => select.into.is_some() || !select.locks.is_empty(),
        _ => false,
    };
    Ok(RawQueryShape {
        root_key,
        node_keys,
        writes_or_locks,
    })
}
