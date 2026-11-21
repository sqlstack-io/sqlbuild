//! The one combined line captured build output prints while nodes run for a long time.

use crate::build_progress::_helpers::text::whole_elapsed;
use crate::build_progress::constants::HEARTBEAT_NAMES;
use crate::build_progress::models::RunningNodeText;
use crate::cli_text::models::CliStyle;
use crate::number_text::main::format_thousands::format_thousands;

/// `… still running (N)` and the longest-running nodes with their elapsed time and, where known, rows so far.
#[must_use]
pub fn heartbeat_message(running: &[RunningNodeText], style: CliStyle) -> String {
    let mut longest: Vec<&RunningNodeText> = running.iter().collect();
    longest.sort_by(|left, right| {
        right
            .elapsed_seconds
            .cmp(&left.elapsed_seconds)
            .then_with(|| left.name.cmp(&right.name))
    });
    let mut parts: Vec<String> = Vec::new();
    for node in longest.iter().take(HEARTBEAT_NAMES) {
        parts.push(node_text(node, style));
    }
    if longest.len() > HEARTBEAT_NAMES {
        parts.push(style.muted(&format!("+{} more", longest.len() - HEARTBEAT_NAMES)));
    }
    format!(
        "{}  {}",
        style.muted(&format!("… still running ({})", running.len())),
        parts.join(" · ")
    )
}

fn node_text(node: &RunningNodeText, style: CliStyle) -> String {
    let rows: String = match node.rows_so_far {
        Some(1) => " (1 row)".to_owned(),
        Some(rows) => format!(" ({} rows)", format_thousands(i128::from(rows))),
        None => String::new(),
    };
    format!(
        "{} {}{rows}",
        style.object_name(&node.name),
        whole_elapsed(node.elapsed_seconds)
    )
}
