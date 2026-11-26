//! `Failures:`: each failed item's full detail, the first section after a run.

use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::fields::field_lines;
use crate::run_output::models::FailureEntry;

/// The section with every entry's name, kind and fields, wrapped to `columns`; empty with none.
#[must_use]
pub fn render_failures(entries: &[FailureEntry], columns: usize, style: CliStyle) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = vec![String::new(), style.error_strong("Failures:")];
    for entry in entries {
        lines.push(String::new());
        let heading_width: usize =
            2 + entry.name.chars().count() + 2 + entry.meta.chars().count() + 2;
        if entry.meta.is_empty() {
            lines.push(format!("  {}", style.section(&entry.name)));
        } else if heading_width <= columns {
            lines.push(format!(
                "  {}  {}",
                style.section(&entry.name),
                style.muted(&format!("({})", entry.meta))
            ));
        } else {
            lines.push(format!("  {}", style.section(&entry.name)));
            lines.push(format!("    {}", style.muted(&format!("({})", entry.meta))));
        }
        for field in &entry.fields {
            lines.extend(field_lines(field, columns, style));
        }
    }
    format!("{}\n", lines.join("\n"))
}
