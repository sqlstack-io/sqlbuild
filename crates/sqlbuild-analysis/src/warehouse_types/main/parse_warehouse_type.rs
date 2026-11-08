//! `parse_warehouse_type`: a warehouse type name as SQLQuack's parser for its dialect reads it.

use serde_json::Value;
use sqlquack_syntax::tokens::models::Dialect;
use sqlquack_syntax::{Expression, ParseOptions, parse_strict};

use crate::warehouse_types::constants::{
    BIGQUERY_DIALECT, CAST_KEY, CAST_TARGET_KEY, POSTGRES_DIALECT, SNOWFLAKE_DIALECT,
};

/// The type tree of `spelling` in `dialect`, None without a parser, or why the parser rejects it.
pub fn parse_warehouse_type(spelling: &str, dialect: &str) -> Result<Option<Value>, String> {
    let cast: String = format!("SELECT CAST(NULL AS {spelling})");
    let statements: Vec<Expression> = match dialect {
        POSTGRES_DIALECT => {
            let data_type = sqlquack_syntax_postgres::parse_type(spelling)
                .map_err(|error| format!("{error:?}"))?;
            return serde_json::to_value(data_type)
                .map(Some)
                .map_err(|error| error.to_string());
        }
        SNOWFLAKE_DIALECT => parse_strict(&cast, Dialect::Snowflake, &ParseOptions::default())
            .map_err(|error| format!("{error:?}"))?,
        BIGQUERY_DIALECT => {
            sqlquack_syntax_bigquery::parse_strict(&cast)
                .map_err(|error| format!("{error:?}"))?
                .statements
        }
        _ => return Ok(None),
    };
    let tree: Value = serde_json::to_value(&statements).map_err(|error| error.to_string())?;
    match (statements.len(), cast_targets(&tree).as_slice()) {
        (1, [target]) => Ok(Some((*target).clone())),
        _ => Err(format!("`{spelling}` is not a single type name")),
    }
}

/// Every cast's target type in `tree`, in order.
fn cast_targets(tree: &Value) -> Vec<&Value> {
    let mut targets: Vec<&Value> = Vec::new();
    let mut pending: Vec<&Value> = vec![tree];
    while let Some(node) = pending.pop() {
        match node {
            Value::Object(fields) => {
                for (key, value) in fields.iter().rev() {
                    pending.push(value);
                    if key == CAST_KEY
                        && let Some(target) = value.get(CAST_TARGET_KEY)
                    {
                        targets.push(target);
                    }
                }
            }
            Value::Array(items) => pending.extend(items.iter().rev()),
            _ => {}
        }
    }
    targets
}
