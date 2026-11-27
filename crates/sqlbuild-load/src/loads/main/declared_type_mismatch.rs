//! Whether a declared column type is the type SQLLoad creates for it.

use sqlload::column_types::models::Warehouse;

use crate::loads::_helpers::column_types::{created_type, declarable, pinned_type};
use crate::loads::models::TypeMismatch;

/// The first of `columns` whose declared type SQLLoad creates in `warehouse` as one `same` calls another; types SQLLoad refuses are its own error.
pub fn declared_type_mismatch(
    columns: &[(String, String)],
    warehouse: Warehouse,
    same: impl Fn(&str, &str) -> bool,
) -> Option<TypeMismatch> {
    columns.iter().find_map(|(column, declared)| {
        let created: String = created_type(declared, warehouse)?;
        (!same(pinned_type(declared), &created)).then(|| TypeMismatch {
            column: column.clone(),
            declared: declared.clone(),
            declare: declarable(&created, warehouse),
            created,
        })
    })
}
