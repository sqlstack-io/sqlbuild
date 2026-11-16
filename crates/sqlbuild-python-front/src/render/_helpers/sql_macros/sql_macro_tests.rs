//! CTE-list SQL macro calls in the `WITH` list of a macro-mode SQL test.

use sqlbuild_analysis::sql_macros::main::with_list_sql_macro_calls::with_list_sql_macro_calls;
use sqlbuild_analysis::sql_macros::models::SqlMacroCall;

/// The test's SQL with each `WITH`-list SQL macro call blanked, and the macros those calls name.
pub(crate) fn without_with_list_calls(sql: &str) -> (String, Vec<String>) {
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

fn leading_space(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// Whether the text after a call ends the `WITH` list: nothing, a statement end or `SELECT`.
fn ends_list(text: &str) -> bool {
    let rest: &str = text.trim_start();
    let word: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    rest.is_empty() || rest.starts_with(';') || word.eq_ignore_ascii_case("SELECT")
}

/// Whether byte `offset` falls in one of the blanked spans.
fn is_blanked(offset: usize, blanked: &[(usize, usize)]) -> bool {
    blanked
        .iter()
        .any(|(start, end)| *start <= offset && offset < *end)
}
