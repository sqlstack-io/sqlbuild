//! The audit summary footer.

use crate::audit_command::models::AuditCounts;
use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::{CliStyle, SummaryCount};

/// A blank line and the PASS, WARN, FAIL, INSUFFICIENT and TOTAL counts.
#[must_use]
pub fn render_audit_summary(counts: AuditCounts, style: CliStyle) -> String {
    format!(
        "\n{}\n",
        format_summary_footer(
            style,
            &[
                SummaryCount::of("PASS", counts.passed),
                SummaryCount::of("WARN", counts.warned),
                SummaryCount::of("FAIL", counts.failed),
                SummaryCount::of("INSUFFICIENT", counts.insufficient),
                SummaryCount::of("TOTAL", counts.total),
            ],
            None,
        )
    )
}
