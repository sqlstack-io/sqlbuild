//! The pieces of an item's result: its row, its child rows and its error, sized to the terminal.

use crate::cli_text::main::format_inline_error_lines::format_inline_error_lines;
use crate::cli_text::main::format_tree_connector::format_tree_connector;
use crate::cli_text::models::CliStyle;
use crate::nested_progress::constants::{
    CHILD_GLYPH_INDENT, CHILD_NAME_OFFSET, DETAIL_MIN_COLUMNS, DETAIL_RESERVE, LABEL_WIDTH,
    MIN_ROW_NAME_WIDTH, RUN_ON_WIDTH, SECONDS_WIDTH, STATUS_WIDTH,
};
use crate::nested_progress::models::{NestedChildRow, NestedItem, NestedLayout};
use crate::run_output::constants::UNWRAPPED_COLUMNS;
use crate::run_output::main::cut_middle::cut_middle;

/// Where a row's columns fall: the width before the label, and the name column's width.
#[derive(Clone, Copy)]
pub(crate) struct RowGeometry {
    pub(crate) prefix_width: usize,
    pub(crate) name_width: usize,
}

/// The counter, then the columns' widths for this terminal.
pub(crate) fn row_geometry(item: &NestedItem, layout: &NestedLayout) -> (String, RowGeometry) {
    let digits: usize = layout.total.max(1).to_string().len();
    let counter: String = format!("{:>digits$}/{}", item.position, layout.total.max(1));
    let prefix_width: usize = 2 + counter.chars().count() + 2;
    let full: usize = layout.name_width + 2;
    let name_width: usize = layout.columns.map_or(full, |columns| {
        let reserve: usize = DETAIL_RESERVE * usize::from(columns >= DETAIL_MIN_COLUMNS);
        let right: usize = STATUS_WIDTH + 1 + RUN_ON_WIDTH + SECONDS_WIDTH + reserve;
        full.min(columns.saturating_sub(prefix_width + LABEL_WIDTH + right))
            .max(MIN_ROW_NAME_WIDTH)
    });
    (
        counter,
        RowGeometry {
            prefix_width,
            name_width,
        },
    )
}

/// `N/M  label  name  STATUS  run-on  time  detail`, the name without its group's prefix.
pub(crate) fn item_row(
    item: &NestedItem,
    (group, counter, geometry): (&str, &str, RowGeometry),
    layout: &NestedLayout,
    style: CliStyle,
) -> String {
    let shown: &str = item
        .item_name
        .strip_prefix(&format!("{group}__"))
        .filter(|rest| !group.is_empty() && !rest.is_empty())
        .unwrap_or(&item.item_name);
    let width: usize = geometry.name_width;
    let name: String = format!("{:<width$}", fitted(shown, width, layout));
    let seconds: String = item
        .seconds
        .map_or_else(String::new, |seconds| format!("  {seconds:>6.2}s"));
    let room: bool = layout
        .columns
        .is_none_or(|columns| columns >= DETAIL_MIN_COLUMNS);
    let detail: String = if room && !item.detail.is_empty() {
        style.muted(&format!("  {}", item.detail))
    } else {
        String::new()
    };
    let row: String = format!(
        "  {counter}  {}{name}{} {}{}{detail}",
        style.muted(&format!("{:<LABEL_WIDTH$}", layout.label)),
        padded_status(&item.status_text, style),
        style.muted(&format!("{:<RUN_ON_WIDTH$}", item.run_on)),
        style.muted(&seconds),
    );
    format!("{}\n", row.trim_end())
}

/// `name` cut in the middle to its column in a terminal; in full when the width is unknown.
fn fitted(name: &str, width: usize, layout: &NestedLayout) -> String {
    match layout.columns {
        Some(_) => cut_middle(name, width.saturating_sub(2)),
        None => name.to_owned(),
    }
}

fn padded_status(status: &str, style: CliStyle) -> String {
    format!(
        "{}{}",
        style.status(status),
        " ".repeat(STATUS_WIDTH.saturating_sub(status.chars().count()))
    )
}

/// One line per child row, each a tree branch, the last closing the tree unless an error follows.
pub(crate) fn child_lines(
    item: &NestedItem,
    (geometry, layout): (RowGeometry, &NestedLayout),
    style: CliStyle,
) -> String {
    let indent: String = " ".repeat(geometry.prefix_width + CHILD_GLYPH_INDENT);
    let width: usize = (geometry.name_width + LABEL_WIDTH)
        .saturating_sub(CHILD_GLYPH_INDENT + CHILD_NAME_OFFSET)
        .max(1);
    let count: usize = item.child_rows.len();
    item.child_rows
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let last: bool = index + 1 == count && item.error.message.is_none();
            child_line(child, (&indent, last), (width, layout), style)
        })
        .collect()
}

fn child_line(
    child: &NestedChildRow,
    (indent, last): (&str, bool),
    (width, layout): (usize, &NestedLayout),
    style: CliStyle,
) -> String {
    let line: String = format!(
        "{indent}{} {}{:<width$}{} {}",
        format_tree_connector(style, last),
        style.muted(&format!("{:<LABEL_WIDTH$}", child.label)),
        fitted(&child.name, width, layout),
        padded_status(&child.status_text, style),
        style.muted(&child.detail)
    );
    format!("{}\n", line.trim_end())
}

/// The error as the last tree child, under a dim `error` label, cut to the terminal.
pub(crate) fn error_lines(
    item: &NestedItem,
    geometry: RowGeometry,
    layout: &NestedLayout,
    style: CliStyle,
) -> String {
    let Some(message) = &item.error.message else {
        return String::new();
    };
    let indent: String = " ".repeat(geometry.prefix_width + CHILD_GLYPH_INDENT);
    let content_indent: usize = geometry.prefix_width + CHILD_GLYPH_INDENT + CHILD_NAME_OFFSET;
    let lines: Vec<String> = format_inline_error_lines(
        (
            item.error.code.as_deref(),
            message,
            item.error.help.as_deref(),
        ),
        layout
            .columns
            .unwrap_or(UNWRAPPED_COLUMNS)
            .saturating_sub(content_indent)
            .max(1),
        style,
    );
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| match index {
            0 => format!(
                "{indent}{} {}{line}\n",
                format_tree_connector(style, true),
                style.error_muted(&format!("{:<LABEL_WIDTH$}", "error"))
            ),
            _ => format!("{}{line}\n", " ".repeat(content_indent)),
        })
        .collect()
}
