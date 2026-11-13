//! Rows named by their key columns; the caller drops any extra row a collation also matches.

/// `rows` holds rendered literals in `key_columns` order, with `None` for SQL NULL; it is never empty.
#[must_use]
pub fn read_keys_sql(
    qualified_name: &str,
    (columns, has_event_id): (&[&str], bool),
    key_columns: &[&str],
    rows: &[Vec<Option<String>>],
) -> String {
    let mut projection = columns.join(", ");
    projection.push_str(if has_event_id {
        ", event_id"
    } else {
        ", NULL AS event_id"
    });
    let mut matches: Vec<String> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut terms: Vec<String> = Vec::with_capacity(key_columns.len());
        for (column, literal) in key_columns.iter().zip(row) {
            terms.push(match literal {
                Some(literal) => format!("{column} = {literal}"),
                None => format!("{column} IS NULL"),
            });
        }
        matches.push(format!("({})", terms.join(" AND ")));
    }
    format!(
        "SELECT {projection} FROM {qualified_name} WHERE {}",
        matches.join(" OR ")
    )
}
