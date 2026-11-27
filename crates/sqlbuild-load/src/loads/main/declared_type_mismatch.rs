//! Whether a declared column type is the type SQLLoad creates for it.

use sqlload::adapters::contract::types::SqlAdapter;
use sqlload::adapters::dispatch::main::dialect::sql_adapter::sql_adapter;
use sqlload::column_types::main::parse_type::parse_type;
use sqlload::column_types::models::Warehouse;

use crate::loads::models::TypeMismatch;

/// The first of `columns` whose declared type SQLLoad creates in `warehouse` as one `same` calls another; types SQLLoad refuses are its own error.
pub fn declared_type_mismatch(
    columns: &[(String, String)],
    warehouse: Warehouse,
    same: impl Fn(&str, &str) -> bool,
) -> Option<TypeMismatch> {
    let adapter: &dyn SqlAdapter = sql_adapter(warehouse);
    columns.iter().find_map(|(column, declared)| {
        let created: String = match parse_type(declared, &[warehouse]) {
            Ok(parsed) => adapter.render_loader_logical_type(&parsed.column_type).sql,
            Err(_refused) => return None,
        };
        (!same(declared, &created)).then(|| TypeMismatch {
            column: column.clone(),
            declared: declared.clone(),
            created,
        })
    })
}
