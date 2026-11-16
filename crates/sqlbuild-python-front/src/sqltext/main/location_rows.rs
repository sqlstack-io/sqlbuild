//! Location rows.

use crate::sqltext::_helpers::model_headers;
use crate::sqltext::models::ColumnLocation;

/// Location rows.
#[inline]
pub fn location_rows(
    locations: Vec<(String, sqlbuild_sqltext::compiler::models::TextLocation)>,
) -> Vec<ColumnLocation> {
    model_headers::location_rows(locations)
}
