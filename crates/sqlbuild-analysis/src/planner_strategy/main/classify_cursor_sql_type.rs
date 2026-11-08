//! The cursor type a warehouse SQL type supports.

use serde_json::{Map, Value};

use crate::planner_strategy::_helpers::decimal_scale::decimal_scale_is_zero;
use crate::planner_strategy::_helpers::type_names::{argument, named_type};
use crate::planner_strategy::constants::{GENERIC_DIALECT, POLYGLOT_CUSTOM_TYPE_NAME};
use crate::planner_strategy::errors::TypeParseFailure;
use crate::planner_strategy::models::CursorType;
use crate::type_system::main::type_args::type_args;

/// Timestamp or integer for supported cross-adapter types; `Err` when an unknown type does not parse.
pub fn classify_cursor_sql_type(sql_type: &str) -> Result<Option<CursorType>, TypeParseFailure> {
    let normalized: String = sql_type
        .to_uppercase()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    if let Some(integral) = decimal_scale_is_zero(&normalized) {
        return Ok(integral.then_some(CursorType::Integer));
    }
    let base: String = normalized
        .split('(')
        .next()
        .unwrap_or_default()
        .trim()
        .replace(' ', "_");
    if let Some(cursor_type) = named_type(&base) {
        return Ok(Some(cursor_type));
    }
    let args: Map<String, Value> =
        type_args(&normalized, GENERIC_DIALECT).map_err(|message| TypeParseFailure {
            normalized_type: normalized.clone(),
            message,
        })?;
    let mut type_name: String = argument(&args, "data_type");
    if type_name == POLYGLOT_CUSTOM_TYPE_NAME {
        type_name = argument(&args, "name").replace('_', "");
    }
    Ok(named_type(&type_name))
}
