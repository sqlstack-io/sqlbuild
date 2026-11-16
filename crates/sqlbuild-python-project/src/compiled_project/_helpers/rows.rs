//! Positional rows shared by the compiled views and the input loops that fill them.

use crate::compiled_project::models::ReferenceTuple;
use sqlbuild_project::models::ReferenceRow;

/// A reference as the tuple Python reads.
pub(crate) fn reference_row_tuple(row: ReferenceRow) -> ReferenceTuple {
    let ReferenceRow {
        kind,
        name,
        package,
        call_argument_count,
    } = row;
    ReferenceTuple {
        kind,
        name,
        package,
        call_argument_count,
    }
}
