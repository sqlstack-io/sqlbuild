use crate::sql_macros::_helpers::parsing::lexing::{Token, tokens};
use crate::sql_macros::_helpers::rendering::placement::CallSite;
use crate::sql_macros::main::sql_macro_calls::sql_macro_calls;
use crate::sql_macros::models::{CallPosition, SqlMacroCall};

/// The SQL macro calls standing between the CTEs of the top-level `WITH` list of `text`.
#[must_use]
pub fn with_list_sql_macro_calls(text: &str) -> Vec<SqlMacroCall> {
    let Ok(calls) = sql_macro_calls(text, None) else {
        return Vec::new();
    };
    if calls.is_empty() {
        return calls;
    }
    let Ok(scanned) = tokens(text) else {
        return Vec::new();
    };
    let Some(first) = scanned
        .first()
        .filter(|token: &&Token| token.is_word(text, "WITH"))
    else {
        return Vec::new();
    };
    let with_start: usize = first.start;
    calls
        .into_iter()
        .filter(|call| {
            let site: CallSite<'_> =
                CallSite::new(text, &scanned, (call.start, call.end), call.end);
            site.position() == CallPosition::CteList
                && site
                    .clause()
                    .is_some_and(|(index, _)| scanned[index].start == with_start)
        })
        .collect()
}
