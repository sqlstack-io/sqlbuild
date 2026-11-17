//! The test's SQL with each `WITH`-list SQL macro call blanked, and the macros those calls name.

use crate::render::_helpers::sql_macros::sql_macro_tests;

/// The test's SQL with each `WITH`-list SQL macro call blanked, and the macros those calls name.
#[inline]
pub fn without_with_list_calls(sql: &str) -> (String, Vec<String>) {
    sql_macro_tests::without_with_list_calls(sql)
}
