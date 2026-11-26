//! What a build prints after it runs: warnings, failures, not run, skipped, failed, then the completion line last.

use crate::build_progress::_helpers::footer::{
    FooterCounts, count_results, failed_items, failure_details, ignored_failure_details,
    not_run_details, python_failure_details, skipped_rollup, warning_details,
};
use crate::build_progress::constants::FAILED;
use crate::build_progress::models::{BuildResultText, PythonNodeText};
use crate::cli_text::main::format_completion_line::format_completion_line;
use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::{CliStyle, CompletionState, SummaryCount};
use crate::run_output::main::render_failed_list::render_failed_list;

fn count(label: &str, value: i64) -> SummaryCount {
    SummaryCount {
        label: label.to_owned(),
        value,
    }
}

/// A successful build's completion label when it let `ignored` audit failures through.
fn ignored_label(ignored: i64) -> String {
    let noun: &str = if ignored == 1 { "failure" } else { "failures" };
    format!("Completed successfully \u{00b7} {ignored} audit {noun} ignored")
}

/// The build footer for `result` and the Python node results run alongside it.
#[must_use]
pub fn format_build_footer(
    result: &BuildResultText,
    python: &[PythonNodeText],
    elapsed: f64,
    style: CliStyle,
) -> String {
    let counts: FooterCounts = count_results(result, python);
    let insufficient: i64 = result.insufficient_count;
    let total: i64 = counts.pass
        + counts.warn
        + counts.fail
        + counts.timeout
        + counts.skip
        + counts.ignored
        + counts.not_run;
    let mut summary_counts: Vec<SummaryCount> = vec![
        count("PASS", counts.pass),
        count("WARN", counts.warn),
        count("FAIL", counts.fail),
    ];
    if counts.timeout > 0 {
        summary_counts.push(count("TIMEOUT", counts.timeout));
    }
    if counts.ignored > 0 {
        summary_counts.push(count("IGNORED", counts.ignored));
    }
    summary_counts.extend([
        count("INSUFFICIENT", insufficient),
        count("SKIP", counts.skip),
    ]);
    if counts.not_run > 0 {
        summary_counts.push(count("NOT_RUN", counts.not_run));
    }
    summary_counts.extend([count("TOTAL", total + insufficient)]);
    let summary: String =
        format_summary_footer(style, &summary_counts, Some(&format!("{elapsed:.2}s")));
    let python_failed: bool = python.iter().any(|node| node.status == FAILED);
    let (state, label): (CompletionState, String) = if result.failed || python_failed {
        (CompletionState::Fail, "Completed with errors".to_owned())
    } else if counts.ignored > 0 {
        (CompletionState::Warn, ignored_label(counts.ignored))
    } else if result.warning_count > 0 || counts.warn > 0 {
        (CompletionState::Warn, "Completed with warnings".to_owned())
    } else {
        (CompletionState::Ok, "Completed successfully".to_owned())
    };
    let mut lines: Vec<String> = warning_details(result, python, style);
    lines.extend(ignored_failure_details(result, style));
    lines.extend(failure_details(result, style));
    lines.extend(python_failure_details(python, style));
    lines.extend(not_run_details(result, style));
    lines.extend(skipped_rollup(result, python, style));
    let failed: Vec<(String, String)> = failed_items(result, python);
    let failed_list: String = render_failed_list(&failed, false, style);
    lines.extend(failed_list.lines().map(str::to_owned));
    lines.push(String::new());
    lines.push(format_completion_line(style, state, &label, Some(&summary)));
    lines.join("\n")
}
