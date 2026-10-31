//! `sqb debug --json` output.

use crate::debug_report::models::{DebugLine, DebugReport};
use crate::json::main::dumps::dumps;
use crate::json::models::{JsonDialect, JsonValue, StdlibJsonOptions};

fn line_value(line: &DebugLine) -> JsonValue {
    let mut fields: Vec<(String, JsonValue)> = vec![
        ("label".to_owned(), JsonValue::String(line.label.clone())),
        (
            "message".to_owned(),
            JsonValue::String(line.message.clone()),
        ),
    ];
    if let Some(status) = line.status {
        fields.push((
            "status".to_owned(),
            JsonValue::String(status.label().to_owned()),
        ));
    }
    if let Some(status_message) = &line.status_message {
        fields.push((
            "status_message".to_owned(),
            JsonValue::String(status_message.clone()),
        ));
    }
    JsonValue::Object(fields)
}

fn lines_value(lines: &[DebugLine]) -> JsonValue {
    JsonValue::Array(lines.iter().map(line_value).collect())
}

/// `json.dumps(payload, sort_keys=True)` of the report and its success flag.
#[must_use]
pub fn format_debug_json(report: &DebugReport) -> String {
    let payload: JsonValue = JsonValue::Object(vec![
        ("success".to_owned(), JsonValue::Bool(report.success())),
        ("runtime".to_owned(), lines_value(&report.runtime)),
        (
            "configuration".to_owned(),
            lines_value(&report.configuration),
        ),
        ("providers".to_owned(), lines_value(&report.providers)),
        ("connection".to_owned(), lines_value(&report.connection)),
        (
            "local_files".to_owned(),
            JsonValue::Array(
                report
                    .local_files
                    .iter()
                    .cloned()
                    .map(JsonValue::String)
                    .collect(),
            ),
        ),
    ]);
    let options: StdlibJsonOptions = StdlibJsonOptions {
        sort_keys: true,
        ..StdlibJsonOptions::new(None)
    };
    dumps(&payload, &JsonDialect::Stdlib(options)).unwrap_or_default()
}
