//! The test summary footer and failure details.

use crate::cli_text::main::format_coded_error::format_coded_error;
use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::{CliStyle, SummaryCount};
use crate::run_on::main::run_on_split::run_on_split;
use crate::test_command::constants::{DEFAULT_ERROR, ERROR_LABEL, ERROR_LABEL_WIDTH, TIMEOUT_CODE};
use crate::test_command::models::TestFailure;
use crate::text::main::python_splitlines::python_splitlines;

fn count(label: &str, value: usize) -> SummaryCount {
    SummaryCount {
        label: label.to_owned(),
        value: i64::try_from(value).unwrap_or(i64::MAX),
    }
}

fn failure_lines(failure: &TestFailure, style: CliStyle) -> Vec<String> {
    let message: &str = failure
        .error_message
        .as_deref()
        .filter(|text| !text.is_empty())
        .unwrap_or(DEFAULT_ERROR);
    let rendered: String = match &failure.error_code {
        Some(code) => {
            format_coded_error((code, message, failure.error_help.as_deref()), false, style)
        }
        None => message.to_owned(),
    };
    let mut split: Vec<&str> = python_splitlines(&rendered);
    if split.is_empty() {
        split.push(&rendered);
    }
    let mut lines: Vec<String> = vec![format!("  {}", failure.label)];
    let label: String = style.error_muted(&format!("{ERROR_LABEL:<ERROR_LABEL_WIDTH$}"));
    for (index, line) in split.iter().enumerate() {
        let shown: String = if index == 0 {
            label.clone()
        } else {
            " ".repeat(ERROR_LABEL_WIDTH)
        };
        lines.push(format!("    {shown}{line}"));
    }
    lines.push(String::new());
    lines
}

/// A blank line, PASS, FAIL and TOTAL counts with the local and warehouse split when given, then
/// the failures when any, and a newline.
#[must_use]
pub fn render_test_completion(
    (passed, total): (usize, usize),
    failures: &[TestFailure],
    split: Option<(usize, usize)>,
    style: CliStyle,
) -> String {
    let timed_out: usize = failures
        .iter()
        .filter(|failure| failure.error_code.as_deref() == Some(TIMEOUT_CODE))
        .count();
    let mut counts: Vec<SummaryCount> = vec![
        count("PASS", passed),
        count("FAIL", total - passed - timed_out.min(total - passed)),
    ];
    if timed_out > 0 {
        counts.push(count("TIMEOUT", timed_out));
    }
    counts.push(count("TOTAL", total));
    let split: Option<String> = split.map(|(local, warehouse)| run_on_split(local, warehouse));
    let summary: String = format_summary_footer(style, &counts, split.as_deref());
    let mut rendered: String = format!("\n{summary}");
    if !failures.is_empty() {
        let mut lines: Vec<String> = vec![style.error_strong("Failures:"), String::new()];
        for failure in failures {
            lines.extend(failure_lines(failure, style));
        }
        rendered = format!("{rendered}\n\n{}", lines.join("\n").trim_end());
    }
    format!("{rendered}\n")
}
