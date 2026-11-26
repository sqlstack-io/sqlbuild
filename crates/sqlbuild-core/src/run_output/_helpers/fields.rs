//! One failure field as lines: a dim label in the type column, then wrapped text or rows.

use crate::cli_text::models::CliStyle;
use crate::run_output::constants::{FIELD_INDENT, FIELD_LABEL_WIDTH, MIN_WRAP_WIDTH};
use crate::run_output::models::{FailureField, FailureRow, RowTone};
use crate::text::main::text_wrap::text_wrap;

const ERROR_LABEL: &str = "error";
const HELP_LABEL: &str = "help";

/// The lines of `field` for a terminal `columns` wide.
pub(crate) fn field_lines(field: &FailureField, columns: usize, style: CliStyle) -> Vec<String> {
    match field {
        FailureField::Error {
            code,
            message,
            help,
        } => {
            let text: String = match code {
                Some(code) => format!("[{code}] {message}"),
                None => message.clone(),
            };
            let mut lines: Vec<String> =
                labelled(&style.error_muted(&pad(ERROR_LABEL)), &text, columns);
            if let Some(help) = help {
                lines.extend(labelled(&style.muted(&pad(HELP_LABEL)), help, columns));
            }
            lines
        }
        FailureField::Labelled { label, text } => {
            labelled(&style.muted(&pad(label)), text, columns)
        }
        FailureField::Verbatim(text) => vec![format!("{}{text}", indent())],
        FailureField::Rows(rows) => rows.iter().map(|row| row_line(row, style)).collect(),
        FailureField::Frame {
            line,
            column,
            width,
            snippet,
        } => {
            let number: String = line.to_string();
            let gutter: String = " ".repeat(number.chars().count());
            let caret_at: usize = usize::try_from((*column - 1).max(0)).unwrap_or(0);
            let carets: String = "^".repeat(usize::try_from((*width).max(1)).unwrap_or(1));
            vec![
                format!(
                    "{}{} {snippet}",
                    indent(),
                    style.link(&format!("{number} |"))
                ),
                format!(
                    "{}{} {}{}",
                    indent(),
                    style.link(&format!("{gutter} |")),
                    " ".repeat(caret_at),
                    style.error(&carets)
                ),
            ]
        }
    }
}

/// `label` then `text`, each line of it wrapped on whitespace under the text column.
fn labelled(label: &str, text: &str, columns: usize) -> Vec<String> {
    let width: usize = columns.saturating_sub(FIELD_INDENT).max(MIN_WRAP_WIDTH);
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let wrapped: Vec<String> = if paragraph.starts_with(' ') || paragraph.contains(" | ") {
            vec![paragraph.to_owned()]
        } else {
            text_wrap(paragraph, width, ("", ""), false)
        };
        for line in if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        } {
            if lines.is_empty() {
                lines.push(format!("    {label}{line}"));
            } else {
                lines.push(format!("{}{line}", indent()));
            }
        }
    }
    lines
}

fn row_line(row: &FailureRow, style: CliStyle) -> String {
    let text: String = match row.tone {
        RowTone::Header => style.muted(&row.text),
        RowTone::Expected | RowTone::Error => style.error(&row.text),
        RowTone::Actual => style.success(&row.text),
        RowTone::Plain => row.text.clone(),
    };
    if row.note.is_empty() {
        return format!("{}{text}", indent());
    }
    format!("{}{text}  {}", indent(), style.muted(&row.note))
}

fn pad(label: &str) -> String {
    format!("{label:<FIELD_LABEL_WIDTH$}")
}

fn indent() -> String {
    " ".repeat(FIELD_INDENT)
}
