//! The test's SQL with each `WITH`-list SQL macro call blanked, and the macros those calls name.

use crate::render::_helpers::sql_macros::sql_macro_tests::ends_list;
use crate::render::_helpers::sql_macros::sql_macro_tests::is_blanked;
use crate::render::_helpers::sql_macros::sql_macro_tests::leading_space;
use sqlbuild_analysis::sql_macros::main::with_list_sql_macro_calls::with_list_sql_macro_calls;
use sqlbuild_analysis::sql_macros::models::SqlMacroCall;

/// The test's SQL with each `WITH`-list SQL macro call blanked, and the macros those calls name.
pub fn without_with_list_calls(sql: &str) -> (String, Vec<String>) {
    let calls: Vec<SqlMacroCall> = with_list_sql_macro_calls(sql);
    if calls.is_empty() {
        return (sql.to_owned(), Vec::new());
    }
    let mut blanked: Vec<(usize, usize)> = Vec::new();
    for call in &calls {
        blanked.push((call.start, call.end));
        let after: usize = call.end + leading_space(&sql[call.end..]);
        if sql[after..].starts_with(',') {
            blanked.push((after, after + 1));
        } else if ends_list(&sql[after..]) {
            let before: &str = sql[..call.start].trim_end();
            if before.ends_with(',') {
                blanked.push((before.len() - 1, before.len()));
            }
        }
    }
    let text: String = sql
        .char_indices()
        .map(|(offset, character)| {
            let inside: bool = is_blanked(offset, &blanked);
            if inside && !character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let names: Vec<String> = calls.into_iter().map(|call| call.name).collect();
    (text, names)
}
