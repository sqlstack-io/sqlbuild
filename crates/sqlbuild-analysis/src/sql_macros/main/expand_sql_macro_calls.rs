use crate::sql_macros::_helpers::parsing::lexing::{Token, tokens};
use crate::sql_macros::constants::INVALID_DEFINITION_CODE;
use crate::sql_macros::errors::ExpansionFailure;
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{ExpandedText, InsertedNames, SqlMacroCall, TextContext};
use crate::sql_macros::types::SqlMacroHost;

/// Each outer SQL macro call's expansion in order, or the index of the call that failed and why.
pub fn expand_sql_macro_calls<H: SqlMacroHost>(
    host: &H,
    context: &TextContext<'_>,
    calls: &[&SqlMacroCall],
) -> Result<Vec<ExpandedText>, (usize, ExpansionFailure<H::Error>)> {
    if calls.is_empty() {
        return Ok(Vec::new());
    }
    let scanned: Vec<Token> = tokens(context.text).map_err(|_| {
        (
            0,
            ExpansionFailure::from(SqlMacroError::new(
                INVALID_DEFINITION_CODE,
                "SQL has an unclosed quote or comment",
            )),
        )
    })?;
    let mut inserted: InsertedNames = InsertedNames::default();
    let mut texts: Vec<ExpandedText> = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().enumerate() {
        let (text, updated) = crate::sql_macros::_helpers::rendering::expansion::expand_call(
            host,
            context,
            (&scanned, inserted),
            call,
        )
        .map_err(|failure| (index, failure))?;
        texts.push(text);
        inserted = updated;
    }
    Ok(texts)
}
