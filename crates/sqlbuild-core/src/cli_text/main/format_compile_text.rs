//! The human `sqb compile` report.

use crate::cli_text::_helpers::diagnostics::diagnostics_text;
use crate::cli_text::_helpers::layout::{
    count_label, fit, phase_line, status_cell, surface_header, tree_connector,
};
use crate::cli_text::constants::{
    HUMAN_MODEL_LIMIT, MAX_MODEL_NAME_WIDTH, MIN_MODEL_NAME_WIDTH, STATUS_WIDTH,
};
use crate::cli_text::models::{CompileTextReport, LineageNote, ModelLine};

/// The report exactly as `format_compile_text` printed it, with its leading and final newline.
#[must_use]
pub fn format_compile_text(report: &CompileTextReport) -> String {
    let style = report.style;
    let mut lines: Vec<String> = vec![
        surface_header(
            style,
            "Compile ready",
            &count_label(report.models.len(), "model"),
        ),
        String::new(),
    ];
    let visible: &[ModelLine] = &report.models[..report.models.len().min(HUMAN_MODEL_LIMIT)];
    let name_width: usize = visible
        .iter()
        .map(|model| model.name.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(MIN_MODEL_NAME_WIDTH, MAX_MODEL_NAME_WIDTH);
    for (index, model) in visible.iter().enumerate() {
        let connector: String = tree_connector(style, index + 1 == visible.len());
        lines.push(format!(
            "{connector} {} {} {}",
            style.object_name(&fit(&model.name, name_width)),
            status_cell(
                style,
                if model.failed { "FAIL" } else { "OK" },
                STATUS_WIDTH
            ),
            style.muted(&format!("{} columns", model.column_count))
        ));
    }
    if report.models.len() > visible.len() {
        lines.push(String::new());
        lines.push(format!(
            "  {}",
            style.muted(&format!(
                "Showing {} of {} models.",
                visible.len(),
                report.models.len()
            ))
        ));
        lines.push(format!(
            "  {}",
            style.muted("Use --json for the full compile report.")
        ));
    }
    lines.push(String::new());
    if let Some(notice) = report
        .semantic_notice
        .as_deref()
        .filter(|notice| !notice.is_empty())
    {
        lines.push(notice.to_owned());
        lines.push(String::new());
    }
    if !report.diagnostics.is_empty() {
        lines.push(diagnostics_text(&report.diagnostics, style));
        lines.push(String::new());
    }
    if let Some(notice) = report.default_rules_notice.as_deref() {
        lines.push(notice.to_owned());
        lines.push(String::new());
    }
    let summary: String = [
        count_label(report.models.len(), "model"),
        count_label(report.seed_count, "seed"),
        count_label(report.function_count, "function"),
        count_label(report.error_count, "error"),
        count_label(report.warning_count, "warning"),
    ]
    .join(", ");
    lines.push(phase_line(
        style,
        report.error_count == 0,
        "Project compiled",
        &summary,
    ));
    for path in &report.failed_model_sql {
        lines.push(format!("  {} {path}", style.muted("Failed SQL:")));
    }
    lines.push(format!(
        "  {} {}/",
        style.muted("Wrote:"),
        report.compiled_dir
    ));
    if report.manifest {
        lines.push(format!("  {} target/manifest.json", style.muted("Wrote:")));
    }
    match report.lineage {
        LineageNote::Rich => lines.push(format!(
            "  {} rich analysis complete; inspect with sqb lineage <model>[.<column>]",
            style.muted("Column lineage:")
        )),
        LineageNote::Unavailable => {
            lines.push(format!(
                "  {} unavailable",
                style.warning("Column lineage:")
            ));
        }
        LineageNote::Silent => {}
    }
    format!("\n{}\n", lines.join("\n"))
}
