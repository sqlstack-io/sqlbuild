//! The unmerged MIN/MAX statement of one physical cursor read.

use crate::planner_warehouse::models::PhysicalCursorQuery;

/// Select the requested bounds of the cursor column as text, each `MIN` or `MAX` rendered by `bound_text`.
pub fn cursor_query_sql(
    query: &PhysicalCursorQuery,
    bound_text: &dyn Fn(&str) -> String,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !query.min_tags.is_empty() {
        parts.push(format!("{} AS _min", bound_text("MIN")));
    }
    if !query.max_tags.is_empty() {
        parts.push(format!("{} AS _max", bound_text("MAX")));
    }
    format!("SELECT {} FROM {}", parts.join(", "), query.relation)
}
