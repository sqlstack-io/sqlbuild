//! The model-file positions of characters of a model's expanded query.

use crate::engine::_helpers::project_rows::{
    located_query_map, query_column_offset, query_line_offset,
};
use crate::models::{AuthoredQuery, AuthoredQueryMap, ModelQueryPlace};
use crate::query_facts::models::ModelSource;

/// The 1-based `file` line and column of each of `sql`'s `characters`, as built-in rules map them.
#[must_use]
pub fn query_file_positions(
    sql: &str,
    (file, place): (&str, &ModelQueryPlace),
    characters: &[usize],
) -> Vec<(u64, u64)> {
    let authored_query: &str = match &place.authored_query {
        None => "",
        Some(AuthoredQuery::Range(start, end)) => file.get(*start..*end).unwrap_or_default(),
        Some(AuthoredQuery::Text(text)) => text,
    };
    let map: Option<AuthoredQueryMap> =
        located_query_map(file, authored_query, place.passes.clone());
    let offsets: Vec<usize> = (0..=sql.chars().count()).collect();
    let source: ModelSource<'_> = ModelSource::new(
        sql,
        &offsets,
        (
            query_line_offset(file, authored_query, sql),
            query_column_offset(file, authored_query, sql),
        ),
    )
    .with_authored(
        if authored_query.is_empty() {
            file
        } else {
            authored_query
        },
        map.as_ref(),
    );
    characters
        .iter()
        .map(|character| source.line_column(*character))
        .collect()
}
