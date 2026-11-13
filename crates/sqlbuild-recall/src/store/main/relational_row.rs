//! `relational_row`: a row as relational stores keep it; a lifecycle event leaves out payload fields its own columns already hold.

use std::borrow::Cow;

use serde_json::Value;
use sqlbuild_core::lifecycle_events::constants::DEFAULT_PRODUCER;

use crate::records::_helpers::events::payload_columns;
use crate::records::models::{RecallRow, RecallValue};
use crate::schema::models::Table;

/// The views put the left-out fields back from their columns, so a payload reads as written; the content digest still covers the whole event. Server-owned events keep their payloads, which the server reads back whole.
#[must_use]
pub fn relational_row(row: &RecallRow) -> Cow<'_, RecallRow> {
    let lifecycle = row.value("producer") == Some(&RecallValue::Text(DEFAULT_PRODUCER.to_owned()));
    let Some(RecallValue::Text(payload)) = row
        .value("payload")
        .filter(|_| row.table == Table::Events && lifecycle)
    else {
        return Cow::Borrowed(row);
    };
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(payload) else {
        return Cow::Borrowed(row);
    };
    let held: Vec<&'static str> = payload_columns(&fields)
        .into_iter()
        .filter(|(name, value)| {
            *name != "payload"
                && !matches!(value, RecallValue::Null)
                && row.value(name) == Some(value)
        })
        .map(|(name, _)| name)
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
        if *name == "payload" {
            *value = RecallValue::Text(stripped.clone());
        }
    }
    Cow::Owned(owned)
}
