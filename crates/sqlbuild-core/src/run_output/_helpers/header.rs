//! The parts of a run header: the `Run on` tree, the `note:` line and the `--explain` rows.

use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::causes::cause_list;
use crate::run_output::constants::{
    BRANCH_GLYPH, EXPLAIN_HINT, HEADER_CAUSE_LIMIT, HEADER_LABEL_WIDTH, LAST_GLYPH, LOCAL_NOTE,
    NOTE_PREFIX, ON_THE_WAREHOUSE, RUN_ON_TITLE,
};
use crate::run_output::models::RunHeader;
use crate::text::main::text_wrap::text_wrap;

/// One row of the `Run on` tree: its glyph, label, count and dim detail.
struct TreeRow<'a> {
    glyph: &'a str,
    label: &'a str,
    count: usize,
    detail: String,
}

fn noun(header: &RunHeader, count: usize) -> &str {
    if count == 1 {
        &header.nouns.0
    } else {
        &header.nouns.1
    }
}

/// Under `--run-on local` with items that can't run: where each item runs, then the causes, at most five.
pub(crate) fn run_on_tree(header: &RunHeader, style: CliStyle) -> Vec<String> {
    if !header.local_only || header.not_run == 0 {
        return Vec::new();
    }
    let causes: usize = header.causes.len();
    let cause_text: String = if causes == 1 {
        "1 cause, listed after the run".to_owned()
    } else {
        format!("{causes} causes, listed after the run")
    };
    let mut lines: Vec<String> = vec![
        String::new(),
        style.section(&format!("{RUN_ON_TITLE} ({})", header.selected)),
    ];
    let rows: [TreeRow<'_>; 2] = [
        TreeRow {
            glyph: BRANCH_GLYPH,
            label: "local",
            count: header.local,
            detail: LOCAL_NOTE.to_owned(),
        },
        TreeRow {
            glyph: LAST_GLYPH,
            label: "can't run locally",
            count: header.not_run,
            detail: cause_text,
        },
    ];
    lines.extend(rows.iter().map(|row| {
        format!(
            "{} {:<HEADER_LABEL_WIDTH$}{:>5}   {}",
            style.muted(row.glyph),
            row.label,
            row.count,
            style.muted(&row.detail)
        )
    }));
    lines.extend(cause_rows(header, style));
    lines
}

fn cause_rows(header: &RunHeader, style: CliStyle) -> Vec<String> {
    let causes: usize = header.causes.len();
    let shown: usize = causes.min(HEADER_CAUSE_LIMIT);
    let width: usize = header.causes[..shown]
        .iter()
        .map(|cause| cause.label.chars().count() + 1)
        .max()
        .unwrap_or(0)
        .max(HEADER_LABEL_WIDTH - 4);
    let mut lines: Vec<String> = Vec::new();
    for (index, cause) in header.causes[..shown].iter().enumerate() {
        let glyph: &str = if index + 1 == shown && shown == causes {
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
        ));
    }
    if shown < causes {
        lines.push(format!(
            "    {} {}",
            style.muted(LAST_GLYPH),
            style.muted(&format!("... and {} more", causes - shown))
        ));
    }
    lines
}

/// In `auto` with items going to the warehouse: `  note: N items run on the warehouse: <causes> (--explain for where)`.
pub(crate) fn auto_note(header: &RunHeader, columns: usize, style: CliStyle) -> Vec<String> {
    if header.local_only || header.causes.is_empty() {
        return Vec::new();
    }
    let verb: &str = if header.warehouse == 1 { "runs" } else { "run" };
    let text: String = format!(
        "{} {} {verb} on the warehouse: {} {EXPLAIN_HINT}",
        header.warehouse,
        noun(header, header.warehouse),
        cause_list(&header.causes)
    );
    let width: usize = columns.saturating_sub(NOTE_PREFIX.len()).max(30);
    text_wrap(&text, width, ("", ""), false)
        .iter()
        .enumerate()
        .map(|(index, line)| match index {
            0 => format!("  {} {line}", style.value("note:")),
            _ => format!("{}{line}", " ".repeat(NOTE_PREFIX.len())),
        })
        .collect()
}

/// With `--explain`: `On the warehouse (N items)` and one `[N] reason  place` row per place.
pub(crate) fn explain_rows(header: &RunHeader, style: CliStyle) -> Vec<String> {
    if header.explain.is_empty() {
        return Vec::new();
    }
    let count: usize = header.explain.iter().map(|row| row.count).sum();
    let reason_width: usize = header
        .explain
        .iter()
        .map(|row| row.reason.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<String> = vec![
        String::new(),
        style.section(&format!(
            "{ON_THE_WAREHOUSE} ({count} {})",
            noun(header, count)
        )),
    ];
    lines.extend(header.explain.iter().map(|row| {
        format!(
            "  {} {:<reason_width$}  {}",
            style.muted(&format!("[{:>2}]", row.count)),
            row.reason,
            style.muted(&row.place)
        )
    }));
    lines
}
