//! One SQL string's references and stop code, scanned without walking past malformed ones.

use crate::sqltext::_helpers::declaration_references;
use crate::sqltext::_helpers::declaration_references::ScanRow;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// One SQL string's references and stop code, scanned without walking past malformed ones.
#[inline]
pub fn declaration_scan(sql: &str, syntax: Option<&LexicalSyntax>) -> ScanRow {
    declaration_references::declaration_scan(sql, syntax)
}
