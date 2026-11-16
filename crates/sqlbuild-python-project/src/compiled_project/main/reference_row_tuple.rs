//! A reference as the tuple Python reads.

use crate::compiled_project::_helpers::rows;
use crate::compiled_project::models::ReferenceTuple;
use sqlbuild_project::models::ReferenceRow;

/// A reference as the tuple Python reads.
#[inline]
pub fn reference_row_tuple(row: ReferenceRow) -> ReferenceTuple {
    rows::reference_row_tuple(row)
}
