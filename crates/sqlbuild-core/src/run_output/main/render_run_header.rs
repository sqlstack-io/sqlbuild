//! The header before a run: what is selected, where it runs, and why some of it can't run locally.

use crate::cli_text::main::format_surface_header::format_surface_header;
use crate::cli_text::models::CliStyle;
use crate::run_output::constants::{
    BRANCH_GLYPH, EXPLAIN_HINT, LAST_GLYPH, ON_THE_WAREHOUSE, RUN_ON_TITLE,
};
use crate::run_output::models::RunHeader;
use crate::text::main::text_wrap::text_wrap;

const LOCAL_NOTE: &str = "DuckDB";
const NOTE_PREFIX: &str = "  note: ";
const LABEL_WIDTH: usize = 22;

/// `<Title> ready  N selected, …`, then the `Run on` tree under `--run-on local`, or a quiet note.
#[must_use]
pub fn render_run_header(header: &RunHeader, columns: usize, style: CliStyle) -> String {
    let (singular, plural) = (&header.nouns.0, &header.nouns.1);
    let noun = |count: usize| if count == 1 { singular } else { plural };
    let mut parts: Vec<String> = vec![format!("{} selected", header.selected)];
    if header.local > 0 {
        parts.push(format!("{} local", header.local));
    }
    if header.warehouse > 0 {
        parts.push(format!("{} on the warehouse", header.warehouse));
    }
    if header.not_run > 0 && header.local_only {
        parts.push(format!("{} can't run locally", header.not_run));
    }
    let mut lines: Vec<String> = vec![format_surface_header(
        style,
        &format!("{} ready", header.title),
        &parts.join(", "),
    )];
    if header.local_only && header.not_run > 0 {
        lines.push(String::new());
        lines.push(style.section(&format!("{RUN_ON_TITLE} ({})", header.selected)));
        let causes: usize = header.causes.len();
        let cause_text: String = if causes == 1 {
            "1 cause, listed after the run".to_owned()
        } else {
            format!("{causes} causes, listed after the run")
        };
        let rows: [(&str, &str, usize, String); 2] = [
            (BRANCH_GLYPH, "local", header.local, LOCAL_NOTE.to_owned()),
            (LAST_GLYPH, "can't run locally", header.not_run, cause_text),
        ];
        for (glyph, label, count, detail) in rows {
            lines.push(format!(
                "{} {label:<LABEL_WIDTH$}{count:>5}   {}",
                style.muted(glyph),
                style.muted(&detail)
            ));
        }
        for (index, cause) in header.causes.iter().enumerate() {
            let glyph: &str = if index + 1 == causes {
                LAST_GLYPH
            } else {
                BRANCH_GLYPH
            };
            lines.push(format!(
                "    {} {:<width$}{:>5}   {}",
                style.muted(glyph),
                cause.label,
                cause.count,
                style.muted(&cause.detail),
                width = LABEL_WIDTH - 4
            ));
        }
    }
    if !header.local_only && !header.causes.is_empty() {
        let causes: Vec<String> = header
            .causes
            .iter()
            .map(|cause| format!("{} {}", cause.label, cause.count))
            .collect();
        let text: String = format!(
            "{} {} run on the warehouse: {} {EXPLAIN_HINT}",
            header.warehouse,
            noun(header.warehouse),
            causes.join(", ")
        );
        let width: usize = columns.saturating_sub(NOTE_PREFIX.len()).max(30);
        for (index, line) in text_wrap(&text, width, ("", ""), false).iter().enumerate() {
            let prefix: String = if index == 0 {
                format!("  {} ", style.value("note:"))
            } else {
                " ".repeat(NOTE_PREFIX.len())
            };
            lines.push(format!("{prefix}{line}"));
        }
    }
    if !header.explain.is_empty() {
        let count: usize = header.explain.iter().map(|row| row.count).sum();
        let reason_width: usize = header
            .explain
            .iter()
            .map(|row| row.reason.chars().count())
            .max()
            .unwrap_or(0);
        lines.push(String::new());
        lines.push(style.section(&format!("{ON_THE_WAREHOUSE} ({count} {})", noun(count))));
        for row in &header.explain {
            lines.push(format!(
                "  {} {:<reason_width$}  {}",
                style.muted(&format!("[{:>2}]", row.count)),
                row.reason,
                style.muted(&row.place)
            ));
        }
    }
    format!("{}\n", lines.join("\n"))
}
