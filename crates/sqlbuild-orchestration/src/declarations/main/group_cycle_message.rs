//! One line per cycle in a job group, then the shared-read hint when a cycle passes through a seed or shared node.

use crate::declarations::constants::SHARED_READ_HINT;
use crate::declarations::models::GroupCycle;

pub fn group_cycle_message(group: &str, cycles: &[GroupCycle]) -> Option<String> {
    if cycles.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = cycles
        .iter()
        .map(|cycle| {
            format!(
                "group dependencies contain a cycle in job group {group}: {}",
                cycle.description
            )
        })
        .collect();
    if cycles.iter().any(|cycle| cycle.shared_read) {
        lines.push(SHARED_READ_HINT.to_owned());
    }
    Some(lines.join("\n"))
}
