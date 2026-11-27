//! Whether a declared column type is the type SQLLoad creates for it.

use sqlload::adapters::contract::types::SqlAdapter;
use sqlload::adapters::dispatch::main::dialect::sql_adapter::sql_adapter;
use sqlload::column_types::main::parse_type::parse_type;
use sqlload::column_types::models::Warehouse;

use crate::loads::constants::{NATIVE_CLOSE, NATIVE_OPEN};
use crate::loads::main::sqlload_column_type::{pinned_type, sqlload_column_type};
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

/// The type SQLLoad creates in `warehouse` for `declared`, or `None` when it refuses it.
#[must_use]
pub fn created_type(declared: &str, warehouse: Warehouse) -> Option<String> {
    let adapter: &dyn SqlAdapter = sql_adapter(warehouse);
    parse_type(&sqlload_column_type(declared, warehouse), &[warehouse])
        .ok()
        .map(|parsed| adapter.render_loader_logical_type(&parsed.column_type).sql)
}

/// `created` when declaring it creates it exactly, else pinned as the warehouse type it is.
fn declarable(created: &str, warehouse: Warehouse) -> String {
    match created_type(created, warehouse) {
        Some(again) if again == created => created.to_owned(),
        _ => format!("{NATIVE_OPEN}{created}{NATIVE_CLOSE}"),
    }
}
