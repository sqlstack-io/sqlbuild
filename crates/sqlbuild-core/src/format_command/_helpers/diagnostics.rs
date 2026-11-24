//! One lint violation as a source-pointing diagnostic block.

use crate::cli_text::models::CliStyle;
use crate::format_command::constants::{
    HELP_CONTINUATION, HELP_PREFIX, REPORT_LINE_WIDTH, TAB_CHARACTER,
};
use crate::format_command::models::LintFinding;
use crate::text::main::cell_len::cell_len;
use crate::text::main::char_cells::char_cells;
use crate::text::main::text_wrap::text_wrap;

fn strong(style: CliStyle, fault: bool, text: &str) -> String {
    if fault {
        style.error_strong(text)
    } else {
        style.warning_strong(text)
    }
}

fn excerpt_end(finding: &LintFinding, length: usize, start: usize) -> usize {
    if finding.end_line == Some(finding.line)
        && let Some(end_column) = finding.end_column
    {
        let end: i64 = (end_column - 1).max(start as i64 + 1);
        return usize::try_from(end).unwrap_or_default().min(length);
    }
    if finding.end_line.is_some() && finding.end_line != Some(finding.line) {
        return length;
    }
    (start + 1).min(length)
}

fn source_excerpt(finding: &LintFinding, source_line: &str, style: CliStyle) -> [String; 4] {
    let characters: Vec<char> = source_line.chars().collect();
    let line_number: String = finding.line.to_string();
    let gutter: String = " ".repeat(line_number.len());
    let start: usize = usize::try_from((finding.column - 1).max(0))
        .unwrap_or_default()
        .min(characters.len());
    let end: usize = excerpt_end(finding, characters.len(), start).max(start);
    let padding: String = characters[..start]
        .iter()
        .map(|character| {
            if *character == TAB_CHARACTER {
                TAB_CHARACTER.to_string()
            } else {
                " ".repeat(char_cells(*character))
            }
        })
        .collect();
    let marked: String = characters[start..end].iter().collect();
    let caret: String = "^".repeat(cell_len(&marked).max(1));
    [
        style.muted(&format!("{gutter} |")),
        format!("{} {source_line}", style.muted(&format!("{line_number} |"))),
        format!(
            "{} {padding}{}",
            style.muted(&format!("{gutter} |")),
            strong(style, finding.fault, &caret)
        ),
        style.muted(&format!("{gutter} |")),
    ]
}

pub(crate) fn help_lines(remediation: &str, style: CliStyle) -> Vec<String> {
    let wrapped: Vec<String> = text_wrap(
        remediation,
        REPORT_LINE_WIDTH,
        (HELP_PREFIX, HELP_CONTINUATION),
        true,
    );
    let Some((first, rest)) = wrapped.split_first() else {
        return Vec::new();
    };
    let first: &str = first.strip_prefix(HELP_PREFIX).unwrap_or(first);
    let mut lines: Vec<String> = vec![format!("  {} {first}", style.muted("= help:"))];
    lines.extend(rest.iter().cloned());
    lines
}

/// The diagnostic block of one violation.
pub(crate) fn violation_block(finding: &LintFinding, style: CliStyle) -> String {
    let severity: &str = if finding.fault { "error" } else { "warning" };
    let label: String = format!("{severity}[{}]", finding.code);
    let mut lines: Vec<String> = vec![
        format!(
            "{}: {}",
            strong(style, finding.fault, &label),
            finding.message
        ),
        style.muted(&format!(
            " --> {}:{}:{}",
            finding.display_path, finding.line, finding.column
        )),
    ];
    if let Some(source_line) = &finding.source_line {
        lines.extend(source_excerpt(finding, source_line, style));
    }
    if let Some(remediation) = &finding.remediation {
        lines.extend(help_lines(remediation, style));
    }
    lines.join("\n")
}
