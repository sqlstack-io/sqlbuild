//! Location rows.

use crate::sqltext::models::ColumnLocation;

/// Location rows.
pub fn location_rows(
    locations: Vec<(String, sqlbuild_sqltext::compiler::models::TextLocation)>,
) -> Vec<ColumnLocation> {
    locations
        .into_iter()
        .map(|(name, location)| ColumnLocation {
            name,
            line: location.line,
            column: location.column,
            end_line: location.end_line,
            end_column: location.end_column,
        })
        .collect()
}
