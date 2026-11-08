//! Type trees from SQLQuack's parsers: Postgres's type grammar, or a cast target's type.

use serde_json::Value;
use sqlquack_syntax::tokens::models::Dialect;
use sqlquack_syntax::{Expression, ParseOptions, parse_strict};

use crate::warehouse_types::constants::{CAST_KEY, CAST_TARGET_KEY};

/// `spelling` read by `sqlquack-syntax-postgres`'s type grammar.
pub(crate) fn postgres_type(spelling: &str) -> Result<Value, String> {
    let data_type =
        sqlquack_syntax_postgres::parse_type(spelling).map_err(|error| format!("{error:?}"))?;
    serde_json::to_value(data_type).map_err(|error| error.to_string())
}

/// `spelling` read as a cast target by SQLQuack's core parser for `dialect`.
pub(crate) fn core_type(spelling: &str, dialect: Dialect) -> Result<Value, String> {
    let statements: Vec<Expression> =
        parse_strict(&cast(spelling), dialect, &ParseOptions::default())
            .map_err(|error| format!("{error:?}"))?;
    single_cast_target(spelling, &statements)
}

/// `spelling` read as a cast target by `sqlquack-syntax-bigquery`.
pub(crate) fn bigquery_type(spelling: &str) -> Result<Value, String> {
    let statements: Vec<Expression> = sqlquack_syntax_bigquery::parse_strict(&cast(spelling))
        .map_err(|error| format!("{error:?}"))?
        .statements;
    single_cast_target(spelling, &statements)
}

/// A statement that casts NULL to `spelling`.
fn cast(spelling: &str) -> String {
    format!("SELECT CAST(NULL AS {spelling})")
}

/// The target type of the one cast in the one statement, or why `spelling` is not one type.
fn single_cast_target(spelling: &str, statements: &[Expression]) -> Result<Value, String> {
    let tree: Value = serde_json::to_value(statements).map_err(|error| error.to_string())?;
    match (statements.len(), cast_targets(&tree).as_slice()) {
        (1, [target]) => Ok((*target).clone()),
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
