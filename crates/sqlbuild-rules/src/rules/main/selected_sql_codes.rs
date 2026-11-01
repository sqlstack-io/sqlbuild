//! `selected_sql_codes`: the built-in SQL Rule codes a `select`/`ignore` pair enables.

use crate::models::RulesCodeGrammar;
use crate::rules::_helpers::catalogue::{matches_any_selector, sql_lint_selection};
use crate::rules::main::catalogue::catalogue;

/// SQL Rule codes `select` enables (SQLClean's defaults when it is `None`) that no `ignore` selector matches, in catalogue order.
#[must_use]
pub fn selected_sql_codes(select: Option<&[String]>, ignore: &[String]) -> Vec<String> {
    let grammar = RulesCodeGrammar;
    sql_lint_selection(&catalogue(), select, &grammar)
        .into_iter()
        .filter(|code| !matches_any_selector(&grammar, code, ignore))
        .collect()
}
