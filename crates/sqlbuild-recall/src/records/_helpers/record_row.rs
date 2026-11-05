//! Decode transported record values against the physical schema, then compute their digest.

use std::collections::BTreeMap;

use crate::records::_helpers::rows::finish_row;
use crate::records::errors::RecordError;
use crate::records::models::{RecallRow, RecallValue, RowEnvelope};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{ColumnType, Table};

/// Envelope and digest columns belong to the receiver, never the transported values.
pub(crate) fn record_row(
    envelope: &RowEnvelope,
    table: Table,
    values: &BTreeMap<String, RecallValue>,
) -> Result<RecallRow, RecordError> {
    let columns = table_spec(table).columns;
    let mut decoded = Vec::with_capacity(values.len());
    for (name, value) in values {
        let column = columns
            .iter()
            .find(|column| column.name == name)
            .ok_or_else(|| RecordError::new(format!("unknown Recall record column {name}")))?;
        let valid = match (value, column.column_type) {
            (RecallValue::Null, _) => column.nullable,
            (RecallValue::Integer(0 | 1), ColumnType::Boolean) => true,
            (RecallValue::Integer(_), ColumnType::Integer) => true,
            (RecallValue::Real(value), ColumnType::Real) => value.is_finite(),
            (RecallValue::Blob(_), ColumnType::Blob) => true,
            (RecallValue::Text(_), ColumnType::Text) => true,
            (RecallValue::Text(value), ColumnType::Json) => {
                serde_json::from_str::<serde_json::Value>(value).map_err(|error| {
                    RecordError::new(format!("invalid Recall JSON column {name}: {error}"))
                })?;
                true
            }
            (RecallValue::Text(value), ColumnType::Timestamp) => {
                chrono::DateTime::parse_from_rfc3339(value).map_err(|error| {
                    RecordError::new(format!("invalid Recall timestamp column {name}: {error}"))
                })?;
                true
            }
            _ => false,
        };
        if !valid {
            return Err(RecordError::new(format!(
                "invalid Recall record column {name}"
            )));
        }
        decoded.push((column.name, value.clone()));
    }
    for column in columns.iter().filter(|column| !column.nullable) {
        if !values.contains_key(column.name) {
            return Err(RecordError::new(format!(
                "missing Recall record column {}",
                column.name
            )));
        }
    }
    Ok(finish_row(table, envelope, decoded))
}
