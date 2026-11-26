//! The widest name a test row shows: its label without the `<group>__` prefix its group header gives.

/// The longest `(label, group)` label as a row shows it.
#[must_use]
pub fn test_name_width(labels: &[(String, String)]) -> usize {
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
}
