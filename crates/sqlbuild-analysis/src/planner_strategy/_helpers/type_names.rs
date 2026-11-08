//! Cursor types of warehouse type names and parsed type arguments.

use serde_json::{Map, Value};
use sqlquack_syntax::traversal::types::ExpressionWalk;
use sqlquack_syntax::{Expression, ParseOptions};

use crate::planner_strategy::constants::{
    INTEGER_TYPE_NAMES, TIMESTAMP_TYPE_NAMES, TYPE_PARSE_DIALECT,
};
use crate::planner_strategy::models::CursorType;

/// The cursor type of a known type name.
pub(crate) fn named_type(name: &str) -> Option<CursorType> {
    if TIMESTAMP_TYPE_NAMES.contains(&name) {
        return Some(CursorType::Timestamp);
    }
    INTEGER_TYPE_NAMES
        .contains(&name)
        .then_some(CursorType::Integer)
}

/// Python's `str()` of one parsed type argument, upper-cased.
pub(crate) fn argument(args: &Map<String, Value>, key: &str) -> String {
    match args.get(key) {
        None => String::new(),
        Some(Value::String(text)) => text.to_uppercase(),
        Some(Value::Null) => "NONE".to_owned(),
        Some(Value::Bool(flag)) => if *flag { "TRUE" } else { "FALSE" }.to_owned(),
        Some(other) => other.to_string().to_uppercase(),
    }
}

/// The scalar arguments of `type_sql` as SQLQuack's parser reads it in a cast, or why not.
pub(crate) fn type_args(type_sql: &str) -> Result<Map<String, Value>, String> {
    let statements: Vec<Expression> = sqlquack_syntax::parse_strict(
        &format!("SELECT CAST(NULL AS {type_sql})"),
        TYPE_PARSE_DIALECT,
        &ParseOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    let data_type = statements
        .iter()
        .flat_map(|statement| statement.dfs())
        .find_map(|node| match node {
            Expression::Cast(cast) => Some(&cast.to),
            _ => None,
        })
        .ok_or_else(|| format!("not one type: {type_sql}"))?;
    match serde_json::to_value(data_type).map_err(|error| error.to_string())? {
        Value::Object(fields) => Ok(fields
            .into_iter()
            .filter(|(_, value)| !matches!(value, Value::Object(_) | Value::Array(_)))
            .collect()),
        other => Err(format!("not one type: {other}")),
    }
}
