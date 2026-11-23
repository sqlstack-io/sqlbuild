use crate::errors::ConfigError;
use crate::header_metadata::main::parse_header_metadata::parse_header_metadata;
use crate::header_metadata::main::parse_schema_columns::parse_schema_columns;
use crate::header_metadata::models::{ColumnRequest, ParsedAudit, ParsedColumn};
use crate::tests::test_types::Value;
use crate::types::AuthoredNode;

/// A mapping from string keys.
pub(super) fn map(entries: &[(&'static str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(key, value)| (Value::Str(key), value.clone()))
            .collect(),
    )
}

/// The model file path the parse errors name.
pub(super) const MODEL_PATH: &str = "/project/models/orders.sql";

/// The comma-separated column names a SCHEMA declaration `orders` parses, or its first error.
pub(super) fn schema_column_summary(columns: &Value) -> String {
    let request = ColumnRequest {
        path: SCHEMA_PATH,
        label: "schema 'orders'",
        require_columns: true,
        allow_migrate_from: false,
    };
    parse_schema_columns(columns, request).map_or_else(stop_summary, |parsed| {
        parsed
            .iter()
            .map(|column| text(Some(&column.name)))
            .collect::<Vec<String>>()
            .join(",")
    })
}

/// The schema file path the parse errors name.
pub(super) const SCHEMA_PATH: &str = "/project/schemas/orders.sql";

/// The parse as one line per column and model audit, or the first error or `unsupported`.
pub(super) fn summary(columns: &Value, audits: &Value) -> Result<Vec<String>, String> {
    let metadata = parse_header_metadata(columns, audits, MODEL_PATH);
    let columns: Vec<String> = metadata
        .columns
        .map_err(stop_summary)?
        .iter()
        .map(column_summary)
        .collect();
    let audits = metadata.audits.map_err(stop_summary)?;
    Ok(columns
        .into_iter()
        .chain(audits.iter().map(audit_summary))
        .collect())
}

fn stop_summary(stop: ConfigError) -> String {
    stop.message
}

fn column_summary(column: &ParsedColumn<Value>) -> String {
    format!(
        "{} {} {} {}",
        text(Some(&column.name)),
        text(column.column_type.as_ref()),
        column
            .nullable
            .as_ref()
            .map_or("-".to_owned(), |value| format!("{value:?}")),
        column
            .audits
            .iter()
            .map(audit_summary)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn audit_summary(audit: &ParsedAudit<Value>) -> String {
    let arguments: Vec<String> = audit
        .arguments
        .iter()
        .map(|(key, _)| text(Some(key)))
        .collect();
    format!(
        "{}:{}{}{}",
        text(Some(&audit.definition_name)),
        arguments.join("+"),
        audit
            .severity
            .as_ref()
            .map_or(String::new(), |severity| format!(
                "@{}",
                text(Some(severity))
            )),
        audit.thresholds.map_or(String::new(), |thresholds| format!(
            " warn={:?} error={:?}",
            thresholds.warn, thresholds.error
        ))
    )
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(AuthoredNode::text)
        .unwrap_or_else(|| "-".to_owned())
}

/// Families as `name:aggregate:pattern`.
pub(super) fn dynamic_summary(raw: &Value) -> Result<Vec<String>, String> {
    crate::header_metadata::main::parse_dynamic_columns::parse_dynamic_columns(
        raw,
        ("orders", "sql/models/orders.sql"),
    )
    .map(|families| {
        families
            .iter()
            .map(|family| {
                format!(
                    "{}:{}:{}",
                    family.name,
                    family.aggregate,
                    family.name_pattern.as_deref().unwrap_or("-")
                )
            })
            .collect()
    })
    .map_err(stop_summary)
}

/// References of `orders` against the discovered `daily_checks` and `freshness` factories.
pub(super) fn factory_summary(raw: &Value) -> Result<Vec<String>, String> {
    crate::header_metadata::main::audit_factory_references::audit_factory_references(
        raw,
        ("orders", "sql/models/orders.sql"),
        &["freshness".to_owned(), "daily_checks".to_owned()],
    )
    .map_err(stop_summary)
}

/// One revenue family, with `pattern` as its `name_pattern` when given.
pub(super) fn family(pattern: Option<&'static str>) -> Value {
    let entries = [
        ("pivot_column", Value::Str("channel")),
        ("value_column", Value::Str("amount")),
        ("aggregate", Value::Str("sum")),
        ("type", Value::Str("DECIMAL")),
    ];
    let pattern = pattern.map(|pattern| ("name_pattern", Value::Str(pattern)));
    map(&entries.into_iter().chain(pattern).collect::<Vec<_>>())
}
