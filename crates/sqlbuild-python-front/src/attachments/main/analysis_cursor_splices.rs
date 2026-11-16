//! `analysis_cursor_sql` with each replaced call as a splice of `sql`.

use crate::attachments::_helpers::cursor_intrinsics;
use sqlbuild_analysis::sql_macros::models::TextSplice;

/// `analysis_cursor_sql` with each replaced call as a splice of `sql`.
#[inline]
pub fn analysis_cursor_splices(
    sql: &str,
    cursor_type: Option<&str>,
) -> Result<(String, Vec<TextSplice>), String> {
    cursor_intrinsics::analysis_cursor_splices(sql, cursor_type)
}
