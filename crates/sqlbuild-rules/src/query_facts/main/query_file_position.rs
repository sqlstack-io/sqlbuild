//! The model-file position of a character of a model's expanded query.

use crate::models::ModelQueryPlace;
use crate::query_facts::models::ModelSource;

/// The 1-based file line and column of `sql`'s character `character`; expansion output maps to its call.
#[must_use]
pub fn query_file_position(sql: &str, place: &ModelQueryPlace, character: usize) -> (u64, u64) {
    let offsets: Vec<usize> = (0..=sql.chars().count()).collect();
    ModelSource::new(sql, &offsets, (place.line_offset, place.column_offset))
        .with_authored(&place.authored_sql, place.authored_query.as_ref())
        .line_column(character)
}
