//! One `UNION ALL` statement reading the bounds of several physical cursor reads.

use crate::planner_warehouse::models::PhysicalCursorQuery;

/// One `(_read, _min, _max)` row per query, each `MIN` or `MAX` rendered by `bound_text`; a missing minimum repeats the maximum, not `NULL`.
pub fn cursor_batch_sql(
    queries: &[&PhysicalCursorQuery],
    bound_text: &dyn Fn(&PhysicalCursorQuery, &str) -> String,
) -> String {
    queries
        .iter()
        .enumerate()
        .map(|(read, query)| {
            let maximum: String = bound_text(query, "MAX");
            let minimum: String = if query.min_tags.is_empty() {
                maximum.clone()
            } else {
                bound_text(query, "MIN")
            };
            format!(
                "SELECT {read} AS _read, {minimum} AS _min, {maximum} AS _max FROM {}",
                query.relation
            )
        })
        .collect::<Vec<String>>()
        .join("\nUNION ALL\n")
}
