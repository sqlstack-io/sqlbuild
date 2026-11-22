//! A titled tree group with key/value rows, bounded to its limit.

use crate::screen_text::_helpers::rows::{column, fit_row, linked, span};
use crate::screen_text::constants::{CUT_MARK, TREE_BAR, TREE_BRANCH, TREE_LAST, TREE_SPACE};
use crate::screen_text::models::{ScreenFacts, Tone, TreeGroup};
use crate::text::main::cell_len::cell_len;

/// `Title (N)`, then one branch per shown entry with its rows, then `… N more (<flag>)`.
#[must_use]
pub fn render_tree_group(group: &TreeGroup, facts: &ScreenFacts) -> Vec<String> {
    let shown: usize = group.entries.len().min(group.limit);
    let hidden: usize = group.total.saturating_sub(shown);
    let mut rows: Vec<String> = vec![tree_row(
        vec![
            span(group.title.as_str(), Tone::Strong),
            span(format!(" ({})", group.total), Tone::Muted),
        ],
        facts,
    )];
    for (index, entry) in group.entries.iter().take(shown).enumerate() {
        let last: bool = index + 1 == shown && hidden == 0;
        rows.push(tree_row(
            vec![
                span(if last { TREE_LAST } else { TREE_BRANCH }, Tone::Muted),
                span(entry.name.as_str(), Tone::Plain),
            ],
            facts,
        ));
        let key_width: usize = entry
            .rows
            .iter()
            .map(|row| cell_len(&row.key))
            .max()
            .unwrap_or(0);
        for (row_index, row) in entry.rows.iter().enumerate() {
            let row_last: bool = row_index + 1 == entry.rows.len();
            rows.push(tree_row(
                vec![
                    span(
                        format!(
                            "{}{}",
                            if last { TREE_SPACE } else { TREE_BAR },
                            if row_last { TREE_LAST } else { TREE_BRANCH }
                        ),
                        Tone::Muted,
                    ),
                    span(format!("{}  ", column(&row.key, key_width)), Tone::Muted),
                    linked(row.value.as_str(), row.tone, row.link.clone()),
                ],
                facts,
            ));
        }
    }
    if hidden > 0 {
        rows.push(tree_row(
            vec![
                span(TREE_LAST, Tone::Muted),
                span(
                    format!("{CUT_MARK} {hidden} more ({})", group.flag),
                    Tone::Muted,
                ),
            ],
            facts,
        ));
    }
    rows
}

/// One row of the group, cut with `…` to the terminal's width.
fn tree_row(segments: Vec<crate::screen_text::models::Segment>, facts: &ScreenFacts) -> String {
    fit_row(segments, facts.columns, facts)
}
