//! The model-file positions of characters of a model's expanded query.

use crate::models::ModelQueryPlace;
use crate::query_facts::models::ModelSource;

/// The 1-based file line and column of each of `sql`'s `characters`; expansion output maps to its call.
#[must_use]
pub fn query_file_positions(
    sql: &str,
    place: &ModelQueryPlace,
    characters: &[usize],
) -> Vec<(u64, u64)> {
    let offsets: Vec<usize> = (0..=sql.chars().count()).collect();
    let source: ModelSource<'_> =
        ModelSource::new(sql, &offsets, (place.line_offset, place.column_offset))
            .with_authored(&place.authored_sql, place.authored_query.as_ref());
    characters
        .iter()
        .map(|character| source.line_column(*character))
        .collect()
}
