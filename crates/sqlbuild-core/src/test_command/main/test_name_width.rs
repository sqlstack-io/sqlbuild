//! The widest name a test row shows: its label without the `<group>__` prefix its group header gives.

use crate::nested_progress::constants::{CHILD_GLYPH_INDENT, CHILD_NAME_OFFSET, LABEL_WIDTH};

/// The longest `(label, group)` label as a row shows it, widened so each child name fits under it.
#[must_use]
pub fn test_name_width(labels: &[(String, String)], child_names: &[String]) -> usize {
    let children: usize = child_names
        .iter()
        .map(|name| name.chars().count() + CHILD_GLYPH_INDENT + CHILD_NAME_OFFSET - LABEL_WIDTH)
        .max()
        .unwrap_or(0);
    labels
        .iter()
        .map(|(label, group)| {
            label
                .strip_prefix(&format!("{group}__"))
                .filter(|rest| !group.is_empty() && !rest.is_empty())
                .unwrap_or(label)
                .chars()
                .count()
        })
        .max()
        .unwrap_or(0)
        .max(children)
}
