//! Python formatting idioms and small labels shared by build progress rows.

use crate::build_progress::constants::{
    ERROR, FAILED, INSUFFICIENT, PASS, ROW_COUNT_ABBREVIATION_THRESHOLD, ROW_COUNT_BILLION,
    ROW_COUNT_MILLION, ROW_COUNT_THOUSAND, SECONDS_PER_MINUTE, SKIPPED, SUCCESS, TIMEOUT_CODE,
    TIMEOUT_OUTCOME, WARN,
};
use crate::build_progress::models::SkipText;
use crate::number_text::main::format_thousands::format_thousands;
use crate::text::main::python_isspace::python_isspace;

/// `f"{text:<width}"`: left-aligned, padded by character count.
pub(crate) fn pad(text: &str, width: usize) -> String {
    let length: usize = text.chars().count();
    format!("{text}{}", " ".repeat(width.saturating_sub(length)))
}

/// `f"{text:>width}"`: right-aligned, padded by character count.
pub(crate) fn pad_left(text: &str, width: usize) -> String {
    let length: usize = text.chars().count();
    format!("{}{text}", " ".repeat(width.saturating_sub(length)))
}

/// `text[:count]` by character.
pub(crate) fn head(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

/// The name cut to `width` characters with a trailing ellipsis.
pub(crate) fn truncate_name(name: &str, width: usize) -> String {
    if name.chars().count() <= width {
        return name.to_owned();
    }
    format!("{}...", head(name, width.saturating_sub(3)))
}

/// `"{counter}/{total}"` right-aligned to twice the total's digits plus one.
pub(crate) fn counter_text(counter: usize, total: usize) -> String {
    pad_left(&format!("{counter}/{total}"), counter_width(total))
}

pub(crate) fn counter_width(total: usize) -> usize {
    total.to_string().len() * 2 + 1
}

/// Seconds with two decimals under a minute, minutes and seconds otherwise.
pub(crate) fn format_duration(duration_ms: Option<f64>) -> String {
    let Some(duration_ms) = duration_ms else {
        return String::new();
    };
    let seconds: f64 = duration_ms / 1000.0;
    if seconds < SECONDS_PER_MINUTE {
        return format!("{seconds:.2}s");
    }
    let minutes: i64 = (seconds / SECONDS_PER_MINUTE).floor() as i64;
    let remaining: f64 = seconds - minutes as f64 * SECONDS_PER_MINUTE;
    format!("{minutes}m{remaining:.1}s")
}

/// The OK, FAIL or SKIP word of an execution status.
pub(crate) fn execution_status_display(status: &str) -> String {
    match status {
        SUCCESS => "OK".to_owned(),
        FAILED => "FAIL".to_owned(),
        SKIPPED => "SKIP".to_owned(),
        other => other.to_owned(),
    }
}

/// The status word of an audit outcome.
pub(crate) fn audit_outcome_display(outcome: &str) -> String {
    match outcome {
        PASS => "PASS".to_owned(),
        WARN => "WARN".to_owned(),
        ERROR => "FAIL".to_owned(),
        INSUFFICIENT => "INSUFFICIENT".to_owned(),
        SKIPPED => "SKIP".to_owned(),
        other => other.to_owned(),
    }
}

/// The status word of a SQL test outcome.
pub(crate) fn test_outcome_display(outcome: &str) -> &'static str {
    match outcome {
        PASS => "PASS",
        ERROR => "ERROR",
        TIMEOUT_OUTCOME => "TIMEOUT",
        _ => "FAIL",
    }
}

/// `TIMEOUT` for a model attempt stopped by its time limit, else its execution status word.
pub(crate) fn model_status_display(status: &str, error_code: Option<&str>) -> String {
    if status == FAILED && error_code == Some(TIMEOUT_CODE) {
        return "TIMEOUT".to_owned();
    }
    execution_status_display(status)
}

/// `  <mode> skip: <reason>` for a skipped row.
pub(crate) fn skip_detail(skip: &SkipText) -> String {
    let label: String = skip
        .mode
        .as_ref()
        .map_or_else(|| "skip".to_owned(), |mode| format!("{mode} skip"));
    match skip.reason.as_deref() {
        Some(reason) if !reason.is_empty() => format!("  {label}: {reason}"),
        _ => format!("  {label}"),
    }
}

/// The SQL with trailing whitespace removed and a terminating semicolon.
pub(crate) fn display_sql(sql: &str) -> String {
    let stripped: &str = sql.trim_end_matches(python_isspace);
    if stripped.is_empty() {
        return sql.to_owned();
    }
    if stripped.ends_with(';') {
        return stripped.to_owned();
    }
    format!("{stripped};")
}

/// A row count with K/M/B abbreviation from ten thousand rows.
pub(crate) fn abbreviated_rows(count: i64) -> String {
    let label: &str = if count == 1 { "row" } else { "rows" };
    if count >= ROW_COUNT_BILLION {
        return format!("{:.1}B {label}", count as f64 / ROW_COUNT_BILLION as f64);
    }
    if count >= ROW_COUNT_MILLION {
        return format!("{:.1}M {label}", count as f64 / ROW_COUNT_MILLION as f64);
    }
    if count >= ROW_COUNT_ABBREVIATION_THRESHOLD {
        return format!("{:.1}K {label}", count as f64 / ROW_COUNT_THOUSAND as f64);
    }
    format!("{} {label}", format_thousands(i128::from(count)))
}
