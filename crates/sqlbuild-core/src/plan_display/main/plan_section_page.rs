//! One page of a plan section's rows, found by name, with what the page's models need beside them.

use serde_json::Value;

use crate::plan_display::_helpers::plan_pages::PlanRows;
use crate::plan_display::_helpers::plan_sections::{matching, page_extras, shown_row};
use crate::plan_display::models::{PlanPage, PlanPageSection};

/// Rows `offset..offset + limit` of the section whose name contains `name`, with their runs, warnings and window starts.
pub fn plan_section_page(
    document: &Value,
    section: PlanPageSection,
    ((offset, limit), name): ((usize, usize), Option<&str>),
) -> PlanPage {
    let plan = PlanRows::new(document);
    let rows = plan.rows(section);
    let found = matching(section, &rows, name);
    let page: Vec<&Value> = found.iter().copied().skip(offset).take(limit).collect();
    let extras = page_extras(&plan, section, &page);
    PlanPage {
        total: found.len(),
        section_total: rows.len(),
        rows: page
            .into_iter()
            .map(|row| shown_row(section, row))
            .collect(),
        runs: extras.runs,
        warnings: extras.warnings,
        cursor_window_starts: extras.cursor_window_starts,
    }
}
