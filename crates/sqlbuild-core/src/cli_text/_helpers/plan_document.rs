//! The full-refresh and incremental bodies of the text plan.

use crate::cli_text::_helpers::layout::surface_header;
use crate::cli_text::_helpers::plan_sections::{
    full_refresh_count_lines, functions_lines, plan_name_width, provider_usage_lines, pruned_lines,
    python_entry_lines, ready_summary, reason_group_lines, remaining_stale_lines,
    routine_model_lines, seed_lines, source_freshness_lines, source_load_lines, warning_lines,
};
use crate::cli_text::_helpers::tree_rows::treeify;
use crate::cli_text::constants::{
    IGNORED_AUDIT_FAILURES_NOTICE, PLAN_READY_TITLE, PRE_SQL_INGRESS_PHASE, PYTHON_INGRESS_TITLE,
    PYTHON_READ_SIDE_TITLE, READ_SIDE_PHASE,
};
use crate::cli_text::models::{CliStyle, ModelPlanText, PlanTextReport};

fn header_lines(report: &PlanTextReport, full_refresh: bool) -> Vec<String> {
    let style: CliStyle = CliStyle { use_color: true };
    let mut lines: Vec<String> = Vec::new();
    if report.include_header {
        lines.push(surface_header(
            style,
            PLAN_READY_TITLE,
            &ready_summary(report, full_refresh),
        ));
    }
    if report.ignore_audit_failures {
        lines.push(style.warning(IGNORED_AUDIT_FAILURES_NOTICE));
    }
    lines
}

/// Every active model is a full refresh: counts by materialization instead of detail.
pub(crate) fn full_refresh_lines(
    report: &PlanTextReport,
    active: &[&ModelPlanText],
) -> Vec<String> {
    let width: usize = plan_name_width(report);
    let mut lines: Vec<String> = header_lines(report, true);
    lines.extend(provider_usage_lines(report));
    lines.extend(python_entry_lines(
        (report, PRE_SQL_INGRESS_PHASE, PYTHON_INGRESS_TITLE),
        width,
    ));
    lines.extend(source_load_lines(report, width));
    lines.extend(functions_lines(report, true, width));
    lines.extend(functions_lines(report, false, width));
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.extend(full_refresh_count_lines(active));
    lines.extend(seed_lines(report));
    lines.extend(python_entry_lines(
        (report, READ_SIDE_PHASE, PYTHON_READ_SIDE_TITLE),
        width,
    ));
    lines.extend(warning_lines(report));
    lines.extend(report.pre_build_lines.iter().cloned());
    lines
}

/// Sections by change reason, drawn as trees.
pub(crate) fn incremental_lines(report: &PlanTextReport, active: &[&ModelPlanText]) -> Vec<String> {
    let width: usize = plan_name_width(report);
    let mut lines: Vec<String> = header_lines(report, false);
    if report.include_freshness_diagnostics {
        lines.extend(source_freshness_lines(report));
    }
    lines.extend(remaining_stale_lines(report));
    lines.extend(provider_usage_lines(report));
    lines.extend(pruned_lines(report));
    lines.extend(report.pre_build_lines.iter().cloned());
    lines.extend(python_entry_lines(
        (report, PRE_SQL_INGRESS_PHASE, PYTHON_INGRESS_TITLE),
        width,
    ));
    lines.extend(source_load_lines(report, width));
    lines.extend(functions_lines(report, true, width));
    lines.extend(reason_group_lines(report, active, width));
    lines.extend(routine_model_lines(report, active, width));
    lines.extend(functions_lines(report, false, width));
    lines.extend(seed_lines(report));
    lines.extend(python_entry_lines(
        (report, READ_SIDE_PHASE, PYTHON_READ_SIDE_TITLE),
        width,
    ));
    let mut lines: Vec<String> = treeify(lines);
    lines.extend(warning_lines(report));
    lines
}
