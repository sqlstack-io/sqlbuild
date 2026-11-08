//! Parse migration SQL with the dialect's parser: SQLQuack where native, else guarded Polyglot.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::lineage::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::planner_migrations::constants::{SET_OPERATION_LEFT, WITH_KEY};
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

/// The one statement of `sql` as a tree; None, the conservative plan, if not read whole.
pub(crate) fn proof_tree(sql: &str, dialect: &str) -> Result<Option<Value>, String> {
    let AnalysisRoute::Native(native) = analysis_route(Some(dialect)) else {
        return parse(sql, dialect);
    };
    let Ok(statement) = native.read_whole(sql) else {
        return Ok(None);
    };
    let mut tree: Value = serde_json::to_value(statement).map_err(|error| error.to_string())?;
    hoist_leading_with(&mut tree);
    Ok(Some(tree))
}

/// Move a set operation's leading WITH from its first SELECT onto the set operation.
fn hoist_leading_with(tree: &mut Value) {
    let Some(operation) = tree
        .as_object_mut()
        .and_then(|object| object.values_mut().next())
        .filter(|operation| operation.get(SET_OPERATION_LEFT).is_some())
    else {
        return;
    };
    if !operation[WITH_KEY].is_null() {
        return;
    }
    let with: Value = leading_with(&mut operation[SET_OPERATION_LEFT]);
    operation[WITH_KEY] = with;
}

/// The WITH of the first SELECT under `node`, taken out of it; null when it has none.
fn leading_with(node: &mut Value) -> Value {
    let Some(inner) = node
        .as_object_mut()
        .and_then(|object| object.values_mut().next())
    else {
        return Value::Null;
    };
    if inner.get(SET_OPERATION_LEFT).is_some() {
        return leading_with(&mut inner[SET_OPERATION_LEFT]);
    }
    inner
        .as_object_mut()
        .and_then(|select| select.insert(WITH_KEY.to_owned(), Value::Null))
        .unwrap_or(Value::Null)
}
