//! Python's `find_macro_call_names` and `find_nested_macro_call_names`.

use crate::macro_calls::_helpers::scanner::{every_call_name, top_level_named_calls};
use crate::macro_calls::errors::ScanError;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Python's `_scan_sqlbuild_call_names`: unique Python and SQL macro call names in encounter order.
pub fn macro_call_names(
    sql: &str,
    nested: bool,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<String>, ScanError> {
    if !sql.contains('@') {
        return Ok(Vec::new());
    }
    let scanned: Vec<String> = if nested {
        every_call_name(sql, syntax)?
    } else {
        let (calls, failure) = top_level_named_calls(sql, syntax);
        if let Some(failure) = failure {
            return Err(failure.error);
        }
        calls.into_iter().map(|call| call.name).collect()
    };
    let mut names: Vec<String> = Vec::new();
    for name in scanned {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}
