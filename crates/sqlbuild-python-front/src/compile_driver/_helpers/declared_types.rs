//! Compile errors for declared column types SQLLoad would create as another type.

use sqlbuild_analysis::analysis_route::main::same_column_type::same_column_type;
use sqlbuild_load::loads::main::declared_type_mismatch::declared_type_mismatch;
use sqlbuild_load::loads::models::{LoadCheck, TypeMismatch};
use sqlbuild_load::warehouses::main::warehouse_dialect::warehouse_dialect;

/// Why `columns`, declared on `owner`, don't match what SQLLoad creates, written as `declare` spells the fix.
pub(crate) fn declared_type_problem(
    check: &LoadCheck,
    (owner, columns): (&str, &[(String, String)]),
    declare: impl Fn(&TypeMismatch) -> String,
) -> Option<String> {
    let dialect: &str = warehouse_dialect(check.warehouse);
    let mismatch: TypeMismatch =
        declared_type_mismatch(columns, check.warehouse, |declared, created| {
            same_column_type(Some(dialect), declared, created)
        })?;
    Some(format!(
        "{owner} declares column {} as {}, which SQLLoad creates as {} in {}; declare {}",
        mismatch.column,
        mismatch.declared,
        mismatch.created,
        check.warehouse,
        declare(&mismatch)
    ))
}
