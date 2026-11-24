//! `On the warehouse (N tests)`: the refusals that sent items to the warehouse, most items first.

use std::collections::BTreeMap;

use crate::cli_text::models::CliStyle;
use crate::run_on::constants::ON_THE_WAREHOUSE_TITLE;
use crate::run_on::models::RunOnDecision;

/// One line per refusal title with how many items it keeps off DuckDB; empty with none.
#[must_use]
pub fn render_warehouse_census(
    decisions: &[RunOnDecision],
    (singular, plural): (&str, &str),
    style: CliStyle,
) -> String {
    let refused: Vec<&RunOnDecision> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .collect();
    if refused.is_empty() {
        return String::new();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for decision in &refused {
        let mut titles: Vec<String> = decision.refusals.iter().map(|refusal| refusal.title()).collect();
        titles.sort();
        titles.dedup();
        for title in titles {
            *counts.entry(title).or_default() += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|(left_title, left), (right_title, right)| {
        right.cmp(left).then_with(|| left_title.cmp(right_title))
    });
    let width: usize = rows.iter().map(|(title, _)| title.chars().count()).max().unwrap_or(0);
    let noun = |count: usize| if count == 1 { singular } else { plural };
    let mut text: String = format!(
        "\n{} ({} {})\n",
        style.section(ON_THE_WAREHOUSE_TITLE),
        refused.len(),
        noun(refused.len())
    );
    for (title, count) in rows {
        text.push_str(&format!("  {title:<width$}  {count} {}\n", noun(count)));
    }
    text
}
