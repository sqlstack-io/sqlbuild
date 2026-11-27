//! The header before a run: what is selected, where it runs, and why some of it can't run locally.

use crate::cli_text::main::format_surface_header::format_surface_header;
use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::header::{auto_note, explain_rows, run_on_tree};
use crate::run_output::models::RunHeader;

/// `<Title> ready  N selected, …`, then the `Run on` tree under `--run-on local`, or a quiet note.
#[must_use]
pub fn render_run_header(header: &RunHeader, columns: usize, style: CliStyle) -> String {
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
    lines.extend(run_on_tree(header, style));
    lines.extend(auto_note(header, columns, style));
    lines.extend(explain_rows(header, style));
    format!("{}\n", lines.join("\n"))
}
