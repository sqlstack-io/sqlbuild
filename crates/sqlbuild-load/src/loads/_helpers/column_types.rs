//! Declared column types as SQLLoad reads them: its own logical types, or the warehouse's own pinned verbatim.

use sqlbuild_analysis::analysis_route::main::names_dialect_type::names_dialect_type;
use sqlload::adapters::contract::types::SqlAdapter;
use sqlload::adapters::dispatch::main::dialect::sql_adapter::sql_adapter;
use sqlload::column_types::main::parse_type::parse_type;
use sqlload::column_types::models::Warehouse;

use crate::loads::constants::{NATIVE_CLOSE, NATIVE_OPEN};
use crate::warehouses::main::warehouse_dialect::warehouse_dialect;

/// `declared` as SQLLoad takes it in `warehouse`: as written when SQLLoad reads it, `native("...")` when only the warehouse knows it, so declared types are literal.
pub(crate) fn sqlload_column_type(declared: &str, warehouse: Warehouse) -> String {
    let written: &str = declared.trim();
    match parse_type(written, &[warehouse]) {
        Ok(_logical) => written.to_owned(),
        Err(_unread) if names_dialect_type(warehouse_dialect(warehouse), written) => {
            pinned(written)
        }
        Err(_refused) => written.to_owned(),
    }
}

/// The type SQLLoad creates in `warehouse` for `declared`, or `None` when it refuses it.
pub(crate) fn created_type(declared: &str, warehouse: Warehouse) -> Option<String> {
    let adapter: &dyn SqlAdapter = sql_adapter(warehouse);
    match parse_type(&sqlload_column_type(declared, warehouse), &[warehouse]) {
        Ok(parsed) => Some(adapter.render_loader_logical_type(&parsed.column_type).sql),
        Err(_refused) => None,
    }
}

/// `created` when declaring it creates it exactly, else pinned as the warehouse type it is.
pub(crate) fn declarable(created: &str, warehouse: Warehouse) -> String {
    match created_type(created, warehouse) {
        Some(again) if again == created => created.to_owned(),
        _ => pinned(created),
    }
}

/// The warehouse type a `native("...")` pin names, or `declared` itself.
pub(crate) fn pinned_type(declared: &str) -> &str {
    let written: &str = declared.trim();
    written
        .strip_prefix(NATIVE_OPEN)
        .and_then(|rest| rest.strip_suffix(NATIVE_CLOSE))
        .unwrap_or(written)
}

/// `warehouse_type` as SQLLoad's verbatim pin.
fn pinned(warehouse_type: &str) -> String {
    format!("{NATIVE_OPEN}{warehouse_type}{NATIVE_CLOSE}")
}
