use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::SqlMacroCall;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The SQL macro calls of `text` not nested in another SQL macro call, in byte offsets.
pub fn sql_macro_calls(
    text: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<SqlMacroCall>, SqlMacroError> {
    if !text.contains(crate::sql_macros::constants::SQL_CALL_PREFIX) {
        return Ok(Vec::new());
    }
    crate::sql_macros::_helpers::rendering::expansion::sql_macro_calls(text, syntax)
}
