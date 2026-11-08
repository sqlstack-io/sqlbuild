//! Parse one raw diff query with the dialect's parser and read its shape: SQLQuack where native,
//! else as the Polyglot wheel's guarded `parse_one` does.

use polyglot_sql::dialects::Dialect;
use polyglot_sql::traversal::ExpressionWalk;
use polyglot_sql::{ComplexityGuardOptions, Expression, ParseOptions};
use sqlquack_syntax::traversal::types::ExpressionWalk as NativeWalk;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::diff_selection::constants::{RAW_QUERY_GUARD, RAW_QUERY_READ_ONLY_ROOTS};
use crate::diff_selection::models::RawQueryShape;

/// The query's shape, or why it is not one statement the dialect's parser reads whole.
pub fn raw_query_shape(sql: &str, dialect: &str) -> Result<RawQueryShape, String> {
    match analysis_route(Some(dialect)) {
        AnalysisRoute::Native(native) => {
            let root: sqlquack_syntax::Expression = native
                .read_whole(sql)
                .map_err(|gap| format!("the {dialect} parser cannot read {}", gap.construct))?;
            native_shape(&root)
        }
        AnalysisRoute::Legacy => polyglot_shape(sql, dialect),
    }
}

/// The shape of a statement SQLQuack read whole.
fn native_shape(root: &sqlquack_syntax::Expression) -> Result<RawQueryShape, String> {
    let root_key: String = root.variant_name().to_lowercase();
    let node_keys: Vec<String> = if RAW_QUERY_READ_ONLY_ROOTS.contains(&root_key.as_str()) {
        NativeWalk::dfs(root)
            .map(|node| node.variant_name().to_lowercase())
            .collect()
    } else {
        Vec::new()
    };
    let writes_or_locks: bool = match root {
        sqlquack_syntax::Expression::Select(select) => {
            select.into.is_some() || !select.locks.is_empty()
        }
        _ => false,
    };
    Ok(RawQueryShape {
        root_key,
        node_keys,
        writes_or_locks,
    })
}

/// The shape Polyglot reads, or the text of the exception the wheel raises.
fn polyglot_shape(sql: &str, dialect: &str) -> Result<RawQueryShape, String> {
    let parser: Dialect =
        Dialect::get_by_name(dialect).ok_or_else(|| format!("Unknown dialect: {dialect}"))?;
    let guard: ComplexityGuardOptions =
        serde_json::from_str(RAW_QUERY_GUARD).map_err(|error| error.to_string())?;
    let mut statements: Vec<Expression> = parser
        .parse_with_options(
            sql,
            &ParseOptions {
                complexity_guard: Some(guard),
            },
        )
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
