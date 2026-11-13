//! `relational_row`: a row as relational stores keep it; a lifecycle event leaves out payload fields its own columns already hold.

use std::borrow::Cow;

use serde_json::{Map, Value};
use sqlbuild_core::lifecycle_events::constants::DEFAULT_PRODUCER;

use crate::records::constants::{PROMOTED_INTEGER_FIELDS, PROMOTED_TEXT_FIELDS};
use crate::records::models::{RecallRow, RecallValue};
use crate::schema::constants::PAYLOAD_COLUMN;
use crate::schema::models::Table;

/// Real fields stay, because a database's JSON functions would not print them back digit for digit. The views put the left-out fields back from their columns, so a payload reads as written; the content digest still covers the whole event. Server-owned events keep their payloads, which the server reads back whole.
#[must_use]
pub fn relational_row(row: &RecallRow) -> Cow<'_, RecallRow> {
    let lifecycle = row.value("producer") == Some(&RecallValue::Text(DEFAULT_PRODUCER.to_owned()));
    let Some(RecallValue::Text(payload)) = row
        .value(PAYLOAD_COLUMN)
        .filter(|_| row.table == Table::Events && lifecycle)
    else {
        return Cow::Borrowed(row);
    };
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(payload) else {
        return Cow::Borrowed(row);
    };
    let held: Vec<&'static str> = PROMOTED_TEXT_FIELDS
        .iter()
        .chain(&PROMOTED_INTEGER_FIELDS)
        .copied()
        .filter(|name| column_holds(row, name, &fields))
        .collect();
    if held.is_empty() {
        return Cow::Borrowed(row);
    }
    for name in held {
        fields.remove(name);
    }
    let stripped: String = Value::Object(fields).to_string();
    let mut owned: RecallRow = row.clone();
    for (name, value) in &mut owned.values {
        if *name == PAYLOAD_COLUMN {
            *value = RecallValue::Text(stripped.clone());
        }
    }
    Cow::Owned(owned)
}

/// Whether the row's `name` column already holds the payload's value of that field.
fn column_holds(row: &RecallRow, name: &str, fields: &Map<String, Value>) -> bool {
    let field: Option<&Value> = fields.get(name);
    match row.value(name) {
        Some(RecallValue::Text(text)) => field.and_then(Value::as_str) == Some(text.as_str()),
        Some(RecallValue::Integer(number)) => field.and_then(Value::as_i64) == Some(*number),
        _ => false,
    }
}
