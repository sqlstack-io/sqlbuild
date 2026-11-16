//! One SQL string's references and stop code, scanned without walking past malformed ones.

use crate::sqltext::_helpers::declaration_references::ScanRow;
use crate::sqltext::_helpers::declaration_references::scan_row;
use crate::sqltext::_helpers::declaration_references::with_sql_macro_calls;
use sqlbuild_analysis::sql_macros::constants::SQL_CALL_PREFIX;
use sqlbuild_sqltext::compiler::main::declaration_references::scan_declaration_references;
use sqlbuild_sqltext::compiler::models::DeclarationReferenceScan;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// One SQL string's references and stop code, scanned without walking past malformed ones.
pub fn declaration_scan(sql: &str, syntax: Option<&LexicalSyntax>) -> ScanRow {
    let scanned: Vec<DeclarationReferenceScan> =
        scan_declaration_references(&[sql.to_owned()], false, syntax);
    let row: ScanRow = scanned.into_iter().next().map(scan_row).unwrap_or_default();
    if !sql.contains(SQL_CALL_PREFIX) {
        return row;
    }
    with_sql_macro_calls(sql, row, syntax)
}
