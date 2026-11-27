//! A completed item's row, sized to the terminal, with its child rows and error when it failed.

use crate::cli_text::models::CliStyle;
use crate::nested_progress::_helpers::item_rows::{
    RowGeometry, child_lines, error_lines, item_row, row_geometry,
};
use crate::nested_progress::constants::PASS_STATUS;
use crate::nested_progress::models::{NestedItem, NestedLayout};

/// `N/M  label  name  STATUS  run-on  time`, then each child and the error as a tree when it failed or `verbose`.
#[must_use]
pub fn render_nested_item(
    item: &NestedItem,
    group: &str,
    layout: &NestedLayout,
    style: CliStyle,
) -> String {
    let (counter, geometry): (String, RowGeometry) = row_geometry(item, layout);
    let row: String = item_row(item, (group, &counter, geometry), layout, style);
    if item.status_text == PASS_STATUS && !layout.verbose {
        return row;
    }
    format!(
        "{row}{}{}",
        child_lines(item, geometry, style),
        error_lines(item, geometry, layout, style)
    )
}
