//! A unified diff's added and removed lines, and whether they differ only in whitespace.

use crate::plan_display::constants::DIFF_HEADER_PREFIXES;
use crate::plan_display::models::DiffStats;

/// The diff's changed lines; whitespace-only when they match with all whitespace out, so case changes count.
pub(crate) fn diff_stats(diff: &[String]) -> DiffStats {
    let mut removed: Vec<&str> = Vec::new();
    let mut added: Vec<&str> = Vec::new();
    for line in diff {
        if DIFF_HEADER_PREFIXES
            .iter()
            .any(|prefix| line.starts_with(prefix))
        {
            continue;
        }
        if let Some(text) = line.strip_prefix('-') {
            removed.push(text);
        } else if let Some(text) = line.strip_prefix('+') {
            added.push(text);
        }
    }
    let changed = !removed.is_empty() || !added.is_empty();
    DiffStats {
        added: added.len(),
        removed: removed.len(),
        whitespace_only: changed && without_whitespace(&removed) == without_whitespace(&added),
    }
}

fn without_whitespace(lines: &[&str]) -> String {
    lines
        .iter()
        .flat_map(|line| line.chars())
        .filter(|character| !character.is_whitespace())
        .collect()
}
