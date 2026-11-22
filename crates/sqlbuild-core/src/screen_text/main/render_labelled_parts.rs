//! A labelled line of parts, such as a plain `warehouse` line, wrapped between parts.

use crate::screen_text::_helpers::rows::{pad, row_width, wrap_words};
use crate::screen_text::constants::SEPARATOR;
use crate::text::main::cell_len::cell_len;

/// `  warehouse  a · b`, continuing under the first part when the parts do not fit.
#[must_use]
pub fn render_labelled_parts(label: &str, parts: &[String], columns: usize) -> Vec<String> {
    let lead: String = format!("  {}  ", pad(label, cell_len(label)));
    let column: usize = cell_len(&lead);
    let width: usize = row_width(&crate::screen_text::models::ScreenFacts {
        columns,
        rows: 0,
        style: crate::cli_text::models::CliStyle { use_color: false },
        links: false,
    })
    .saturating_sub(column)
    .max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut line: String = String::new();
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if line.is_empty() {
            line.clone_from(part);
        } else if cell_len(&line) + cell_len(SEPARATOR) + cell_len(part) <= width {
            line = format!("{line}{SEPARATOR}{part}");
        } else {
            lines.push(std::mem::take(&mut line));
            line.clone_from(part);
        }
        if cell_len(&line) > width {
            let mut wrapped: Vec<String> = wrap_words(&line, width);
            line = wrapped.pop().unwrap_or_default();
            lines.extend(wrapped);
        }
    }
    lines.push(line);
    lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let first: String = if index == 0 {
                lead.clone()
            } else {
                " ".repeat(column)
            };
            format!("{first}{text}")
        })
        .collect()
}
