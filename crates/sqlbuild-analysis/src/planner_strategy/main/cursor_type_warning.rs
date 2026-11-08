//! Whether a cursor column's warehouse type matches the declared cursor type.

use crate::planner_strategy::errors::TypeParseFailure;
use crate::planner_strategy::main::classify_cursor_sql_type::classify_cursor_sql_type;
use crate::planner_strategy::models::{CursorType, CursorTypeCheck, WarningRow, WarningSeverity};

fn cursor_type_name(cursor_type: CursorType) -> &'static str {
    match cursor_type {
        CursorType::Timestamp => "timestamp",
        CursorType::Integer => "integer",
    }
}

/// An error (with SQL analysis) or warning when the warehouse type is the other cursor type, and any parse error that left it unclassified.
pub fn cursor_type_warning(
    check: &CursorTypeCheck,
) -> (Option<WarningRow>, Option<TypeParseFailure>) {
    let (Some(column), Some(declared)) = (&check.cursor_column, &check.cursor_type) else {
        return (None, None);
    };
    let Some((_, warehouse_type)) = check
        .warehouse_columns
        .iter()
        .find(|(name, _)| name.to_uppercase() == column.to_uppercase())
    else {
        return (None, None);
    };
    let declared: CursorType = match declared.as_str() {
        "timestamp" => CursorType::Timestamp,
        "integer" => CursorType::Integer,
        _ => return (None, None),
    };
    let detected: CursorType = match classify_cursor_sql_type(warehouse_type) {
        Ok(Some(detected)) => detected,
        Ok(None) => return (None, None),
        Err(failure) => return (None, Some(failure)),
    };
    if detected == declared {
        return (None, None);
    }
    let (severity, verb) = if check.sql_analysis_enabled {
        (WarningSeverity::Error, "is")
    } else {
        (WarningSeverity::Warning, "appears to be")
    };
    let message: String = format!(
        "cursor column '{column}' has warehouse type '{warehouse_type}' which {verb} {}, but \
         cursor_type is '{}'",
        cursor_type_name(detected),
        cursor_type_name(declared)
    );
    (
        Some(WarningRow {
            severity,
            message,
            code: None,
        }),
        None,
    )
}
