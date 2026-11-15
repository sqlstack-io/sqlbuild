//! Positional rows shared by the compiled views and the input loops that fill them.

use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::positional_record;

positional_record! {
    into;
    /// A reference as the tuple Python reads: kind, name, package and call argument count.
    pub(crate) struct ReferenceTuple {
        pub(crate) kind: String,
        pub(crate) name: String,
        package: Option<String>,
        call_argument_count: Option<usize>,
    }
}

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
