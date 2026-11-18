//! The persistent row of a finished seed, function or source load.

use crate::build_progress::_helpers::error_rows::error_detail;
use crate::build_progress::_helpers::rows::{
    TopLevelRow, event_blocks, prefix_width, top_level_line,
};
use crate::build_progress::_helpers::text::{
    counter_text, execution_status_display, format_duration, skip_detail, truncate_name,
};
use crate::build_progress::constants::{FAILED, SKIPPED};
use crate::build_progress::models::{ErrorText, ProgressLayout, ResourceResult};
use crate::cli_text::models::CliStyle;
use crate::number_text::main::format_thousands::format_thousands;

/// The row at `counter`, its log and SQL blocks, and its inline error.
#[must_use]
pub fn render_resource_result(
    result: ResourceResult<'_>,
    counter: usize,
    layout: ProgressLayout,
    style: CliStyle,
) -> String {
    let counter: String = counter_text(counter, layout.total);
    match result {
        ResourceResult::Node(node) => {
            let mut text: String = top_level_line(
                &TopLevelRow {
                    counter: &counter,
                    resource_type: &node.kind,
                    name: &truncate_name(&node.name, layout.name_width),
                    status: &execution_status_display(&node.status),
                    duration: &format_duration(node.duration_ms),
                    detail: "",
                },
                layout,
                style,
            );
            if node.status == FAILED
                && let Some(message) = node.error.message.as_deref().filter(|m| !m.is_empty())
            {
                text.push_str(&error_detail(
                    (&node.error, message),
                    (prefix_width(layout), layout.columns),
                    false,
                    style,
                ));
            }
            text
        }
        ResourceResult::Load(load) => {
            let detail: String = if load.status == SKIPPED {
                skip_detail(&load.skip)
            } else {
                format!("  rows={}", format_thousands(i128::from(load.rows_loaded)))
            };
            let mut text: String = top_level_line(
                &TopLevelRow {
                    counter: &counter,
                    resource_type: &load.resource_kind,
                    name: &truncate_name(&load.source_name, layout.name_width),
                    status: &execution_status_display(&load.status),
                    duration: &format_duration(load.duration_ms),
                    detail: &detail,
                },
                layout,
                style,
            );
            text.push_str(&event_blocks(&load.events, layout.verbose, style));
            if load.status == FAILED
                && let Some(message) = load.error_message.as_deref().filter(|m| !m.is_empty())
            {
                text.push_str(&error_detail(
                    (&ErrorText::default(), message),
                    (prefix_width(layout), layout.columns),
                    false,
                    style,
                ));
            }
            text
        }
    }
}
