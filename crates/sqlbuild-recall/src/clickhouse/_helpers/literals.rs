//! ClickHouse literals and key expressions for the store's own queries.

use crate::clickhouse::constants::TIMESTAMP_PATTERN;
use crate::records::main::format_timestamp::format_timestamp;
use crate::records::main::parse_timestamp::parse_timestamp;
use crate::records::main::timestamp_micros::timestamp_micros;
use crate::records::models::RecallValue;
use crate::schema::main::table_columns::table_columns;
use crate::schema::models::{ColumnType, Table};

/// A single-quoted ClickHouse string literal.
pub(crate) fn string_literal(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for character in text.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '\'' => quoted.push_str("\\'"),
            other => quoted.push(other),
        }
    }
    quoted.push('\'');
    quoted
}

/// The literal of a key value of `column_type`; timestamps as `DateTime64` microseconds.
pub(crate) fn value_literal(value: &RecallValue, column_type: ColumnType) -> String {
    match (value, column_type) {
        (RecallValue::Text(text), ColumnType::Timestamp) => timestamp_micros(text).map_or_else(
            || string_literal(text),
            |micros| format!("fromUnixTimestamp64Micro({micros}, 'UTC')"),
        ),
        (RecallValue::Text(text), _) => string_literal(text),
        (RecallValue::Integer(number), _) => number.to_string(),
        (RecallValue::Real(number), _) => number.to_string(),
        (RecallValue::Blob(bytes), _) => string_literal(&String::from_utf8_lossy(bytes)),
        (RecallValue::Null, _) => "NULL".to_owned(),
    }
}

/// The type of `column` in `table`.
pub(crate) fn column_type(table: Table, column: &str) -> ColumnType {
    table_columns(table)
        .iter()
        .find(|spec| spec.name == column)
        .map_or(ColumnType::Text, |spec| spec.column_type)
}

/// SQL rendering `column` as the text a conflict's record key holds.
pub(crate) fn key_text_expression(table: Table, column: &str) -> String {
    match column_type(table, column) {
        ColumnType::Timestamp => {
            format!("formatDateTime(`{column}`, '{TIMESTAMP_PATTERN}', 'UTC')")
        }
        _ => format!("toString(`{column}`)"),
    }
}

/// A value's text as a record key holds it: timestamps normalised to Recall's format.
pub(crate) fn key_text(value: Option<&RecallValue>, column_type: ColumnType) -> String {
    match value {
        Some(RecallValue::Text(text)) if column_type == ColumnType::Timestamp => {
            parse_timestamp(text).map_or_else(|| text.clone(), format_timestamp)
        }
        _ => value.map_or_else(String::new, RecallValue::key_text),
    }
}
