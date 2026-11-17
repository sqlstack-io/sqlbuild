//! One `UNION ALL` statement reading the bounds of several physical cursor reads.

use crate::planner_warehouse::models::PhysicalCursorQuery;

/// One `(_read, _min, _max)` row per query; a missing minimum repeats the maximum, not `NULL`.
pub fn cursor_batch_sql(
    queries: &[&PhysicalCursorQuery],
    as_text: &dyn Fn(&PhysicalCursorQuery, &str) -> String,
) -> String {
    queries
        .iter()
        .enumerate()
        .map(|(read, query)| {
            let maximum: String = as_text(query, &format!("MAX({})", query.cursor_column));
            let minimum: String = if query.min_tags.is_empty() {
                maximum.clone()
            } else {
                as_text(query, &format!("MIN({})", query.cursor_column))
            };
            format!(
                "SELECT {read} AS _read, {minimum} AS _min, {maximum} AS _max FROM {}",
                query.relation
            )
        })
        .collect::<Vec<String>>()
        .join("\nUNION ALL\n")
}
