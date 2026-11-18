//! The persistent line of one finished load, with its log blocks and error.

use crate::cli_text::models::CliStyle;
use crate::load_command::_helpers::padding::pad_right;
use crate::load_command::constants::{
    KIND_WIDTH, LOG_CONTINUATION, LOG_LABEL, NAME_WIDTH, STATUS_WIDTH,
};
use crate::load_command::models::{LoadResultText, LoadStatus};
use crate::number_text::main::format_thousands::format_thousands;
use crate::text::main::python_splitlines::python_splitlines;

fn log_block(message: &str, style: CliStyle) -> String {
    let lines: Vec<&str> = python_splitlines(message);
    let first: &str = lines.first().copied().unwrap_or("");
    let mut text: String = format!("\n{}{}\n", style.label(LOG_LABEL), style.muted(first));
    for line in lines.iter().skip(1) {
        text.push_str(&format!(
            "{}\n",
            style.muted(&format!("{LOG_CONTINUATION}{line}"))
        ));
    }
    text
}

/// Position, kind, name, status, duration and rows, then log blocks and the error message.
#[must_use]
pub fn render_load_result(
    result: &LoadResultText,
    (ordinal, total): (i64, i64),
    duration_ms: Option<f64>,
    style: CliStyle,
) -> String {
    let status: String = style.status(match result.status {
        LoadStatus::Success => "OK",
        LoadStatus::Skipped => "SKIP",
        LoadStatus::Failed => "FAIL",
    });
    let duration: String =
        duration_ms.map_or_else(String::new, |ms| format!("{:.2}s", ms / 1000.0));
    let mut text: String = format!(
        "  {ordinal}/{total}  {}{} {} {duration}  rows={}\n",
        pad_right(&result.resource_kind, KIND_WIDTH),
        pad_right(&result.source_name, NAME_WIDTH),
        pad_right(&status, STATUS_WIDTH),
        format_thousands(i128::from(result.rows_loaded))
    );
    for message in &result.log_messages {
        text.push_str(&log_block(message, style));
    }
    if let Some(error) = &result.error_message {
        text.push_str(&style.error(&format!("    {error}\n")));
    }
    text
}
