//! Assemble a row: the dataset's values, the common columns and the content digest.

use crate::records::_helpers::ids::content_hash;
use crate::records::models::{RecallRow, RecallValue, RowEnvelope};
use crate::schema::constants::{ARTIFACT_CONTENT_COLUMNS, RECALL_SCHEMA_VERSION};
use crate::schema::models::Table;

pub(crate) fn text(value: impl Into<String>) -> RecallValue {
    RecallValue::Text(value.into())
}

pub(crate) fn optional_text(value: Option<&str>) -> RecallValue {
    value.map_or(RecallValue::Null, |value| {
        RecallValue::Text(value.to_owned())
    })
}

pub(crate) fn integer(value: u64) -> RecallValue {
    RecallValue::Integer(i64::try_from(value).unwrap_or(i64::MAX))
}

pub(crate) fn boolean(value: bool) -> RecallValue {
    RecallValue::Integer(i64::from(value))
}

/// `values` plus the envelope columns and `content_hash`; an artifact's digest covers only its content, since every invocation that sees it writes the same row.
pub(crate) fn finish_row(
    table: Table,
    envelope: &RowEnvelope,
    mut values: Vec<(&'static str, RecallValue)>,
) -> RecallRow {
    values.push(("invocation_id", text(&envelope.invocation_id)));
    values.push((
        "invocation_started_at",
        text(&envelope.invocation_started_at),
    ));
    values.push((
        "schema_version",
        RecallValue::Integer(RECALL_SCHEMA_VERSION),
    ));
    values.push(("producer_version", text(&envelope.producer_version)));
    let digest: String = match table {
        Table::Artifacts => content_hash(
            &values
                .iter()
                .filter(|(name, _)| ARTIFACT_CONTENT_COLUMNS.contains(name))
                .cloned()
                .collect::<Vec<_>>(),
        ),
        _ => content_hash(&values),
    };
    values.push(("content_hash", RecallValue::Text(digest)));
    RecallRow { table, values }
}
