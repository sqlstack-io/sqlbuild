//! A reference as the tuple Python reads.

use crate::compiled_project::models::ReferenceTuple;
use sqlbuild_project::models::ReferenceRow;

/// A reference as the tuple Python reads.
pub fn reference_row_tuple(row: ReferenceRow) -> ReferenceTuple {
    let ReferenceRow {
        kind,
        name,
        package,
        call_argument_count,
        ..
    } = row;
    ReferenceTuple {
        kind,
        name,
        package,
        call_argument_count,
    }
}
