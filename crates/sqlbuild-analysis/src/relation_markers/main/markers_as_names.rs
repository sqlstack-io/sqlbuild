//! `markers_as_names`: relation marker calls in code as plain names that keep every position.

use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::relation_markers::main::located_markers_as_names::located_markers_as_names;
use crate::relation_markers::types::MarkerCall;

/// `sql` with each marker call in code as `__sqb_<index>`, padded so later text keeps its line and column, and the calls by index.
pub fn markers_as_names(
    sql: &str,
    syntax: &LexicalSyntax,
) -> Result<(String, Vec<MarkerCall>), String> {
    located_markers_as_names(sql, syntax).map(|(named, calls, _)| (named, calls))
}
