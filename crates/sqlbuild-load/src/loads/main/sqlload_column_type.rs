//! A declared column type as SQLLoad reads it: its own logical type, or the warehouse's own type pinned verbatim.

use sqlbuild_analysis::analysis_route::main::names_dialect_type::names_dialect_type;
use sqlload::column_types::main::parse_type::parse_type;
use sqlload::column_types::models::Warehouse;

use crate::loads::constants::{NATIVE_CLOSE, NATIVE_OPEN};
use crate::warehouses::main::warehouse_dialect::warehouse_dialect;

/// `declared` as SQLLoad takes it in `warehouse`: as written when SQLLoad reads it, `native("...")` when only the warehouse knows it, so declared types are literal.
#[must_use]
pub fn sqlload_column_type(declared: &str, warehouse: Warehouse) -> String {
    let written: &str = declared.trim();
    if parse_type(written, &[warehouse]).is_ok()
        || !names_dialect_type(warehouse_dialect(warehouse), written)
    {
        return written.to_owned();
    }
    format!("{NATIVE_OPEN}{written}{NATIVE_CLOSE}")
}

/// The warehouse type a `native("...")` pin names, or `declared` itself.
#[must_use]
pub fn pinned_type(declared: &str) -> &str {
    let written: &str = declared.trim();
    written
        .strip_prefix(NATIVE_OPEN)
        .and_then(|rest| rest.strip_suffix(NATIVE_CLOSE))
        .unwrap_or(written)
}
