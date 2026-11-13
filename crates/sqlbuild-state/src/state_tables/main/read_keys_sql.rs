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
    let matches: Vec<String> = rows
        .iter()
        .map(|row| {
            let terms: Vec<String> = key_columns
                .iter()
                .zip(row)
                .map(|(column, literal)| {
                    literal.as_ref().map_or_else(
                        || format!("{column} IS NULL"),
                        |literal| format!("{column} = {literal}"),
                    )
                })
                .collect();
            format!("({})", terms.join(" AND "))
        })
        .collect();
    format!(
        "SELECT {projection} FROM {qualified_name} WHERE {}",
        matches.join(" OR ")
    )
}
