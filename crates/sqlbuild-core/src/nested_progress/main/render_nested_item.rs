//! A completed item's row, sized to the terminal, with its child rows and error when it failed.

use crate::cli_text::main::format_inline_error_lines::format_inline_error_lines;
use crate::cli_text::main::format_tree_connector::format_tree_connector;
use crate::cli_text::models::CliStyle;
use crate::nested_progress::constants::{
    CHILD_GLYPH_INDENT, CHILD_NAME_OFFSET, DETAIL_MIN_COLUMNS, LABEL_WIDTH, MIN_ROW_NAME_WIDTH,
    PASS_STATUS, RUN_ON_WIDTH, SECONDS_WIDTH, STATUS_WIDTH,
};
use crate::nested_progress::models::{NestedItem, NestedLayout};
use crate::run_output::main::cut_middle::cut_middle;

/// `N/M  label  name  STATUS  run-on  time`, then each child and the error as a tree when it failed or `verbose`.
#[must_use]
pub fn render_nested_item(
    item: &NestedItem,
    group: &str,
    layout: &NestedLayout,
    style: CliStyle,
) -> String {
    let digits: usize = layout.total.max(1).to_string().len();
    let counter: String = format!("{:>digits$}/{}", item.position, layout.total.max(1));
    let prefix_width: usize = 2 + counter.chars().count() + 2;
    let wide: bool = layout.columns >= DETAIL_MIN_COLUMNS;
    let reserve: usize = if wide { "  2 real models".len() } else { 0 };
    let right: usize = STATUS_WIDTH + 1 + RUN_ON_WIDTH + SECONDS_WIDTH + reserve;
    let name_width: usize = (layout.name_width + 2)
        .min(
            layout
                .columns
                .saturating_sub(prefix_width + LABEL_WIDTH + right),
        )
        .max(MIN_ROW_NAME_WIDTH);
    let shown: &str = item
        .item_name
        .strip_prefix(&format!("{group}__"))
        .filter(|rest| !group.is_empty() && !rest.is_empty())
        .unwrap_or(&item.item_name);
    let name: String = format!(
        "{:<name_width$}",
        cut_middle(shown, name_width.saturating_sub(2))
    );
    let seconds: String = item
        .seconds
        .map_or_else(String::new, |seconds| format!("  {seconds:>6.2}s"));
    let detail: String = if wide && !item.detail.is_empty() {
        style.muted(&format!("  {}", item.detail))
    } else {
        String::new()
    };
    let status: String = format!(
        "{}{}",
        style.status(&item.status_text),
        " ".repeat(STATUS_WIDTH.saturating_sub(item.status_text.chars().count()))
    );
    let row: String = format!(
        "  {counter}  {}{name}{status} {}{}{detail}",
        style.muted(&format!("{:<LABEL_WIDTH$}", layout.label)),
        style.muted(&format!("{:<RUN_ON_WIDTH$}", item.run_on)),
        style.muted(&seconds),
    );
    let mut text: String = format!("{}\n", row.trim_end());
    if item.status_text == PASS_STATUS && !layout.verbose {
        return text;
    }
    let indent: String = " ".repeat(prefix_width + CHILD_GLYPH_INDENT);
    let child_width: usize = (name_width + LABEL_WIDTH)
        .saturating_sub(CHILD_GLYPH_INDENT + CHILD_NAME_OFFSET)
        .max(1);
    for (index, child) in item.child_rows.iter().enumerate() {
        let last: bool = index + 1 == item.child_rows.len() && item.error.message.is_none();
        let line: String = format!(
            "{indent}{} {}{:<child_width$}{}{} {}",
            format_tree_connector(style, last),
            style.muted(&format!("{:<LABEL_WIDTH$}", child.label)),
            cut_middle(&child.name, child_width.saturating_sub(2)),
            style.status(&child.status_text),
            " ".repeat(STATUS_WIDTH.saturating_sub(child.status_text.chars().count())),
            style.muted(&child.detail)
        );
        text.push_str(&format!("{}\n", line.trim_end()));
    }
    let Some(message) = &item.error.message else {
        return text;
    };
    let content_indent: usize = prefix_width + CHILD_GLYPH_INDENT + CHILD_NAME_OFFSET;
    let lines: Vec<String> = format_inline_error_lines(
        (
            item.error.code.as_deref(),
            message,
            item.error.help.as_deref(),
        ),
        layout.columns.saturating_sub(content_indent).max(1),
        style,
    );
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            text.push_str(&format!(
                "{indent}{} {}{line}\n",
                format_tree_connector(style, true),
                style.error_muted(&format!("{:<LABEL_WIDTH$}", "error"))
            ));
        } else {
            text.push_str(&format!("{}{line}\n", " ".repeat(content_indent)));
        }
    }
    text
}
