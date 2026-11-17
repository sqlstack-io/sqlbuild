//! `analysis_cursor_sql` with each replaced call as a splice of `sql`.

use crate::attachments::_helpers::cursor_intrinsics::MODEL_SQL_CONTEXT;
use crate::attachments::_helpers::cursor_intrinsics::analysis_bound;
use sqlbuild_analysis::sql_macros::models::TextSplice;
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;

/// `analysis_cursor_sql` with each replaced call as a splice of `sql`.
pub fn analysis_cursor_splices(
    sql: &str,
    cursor_type: Option<&str>,
) -> Result<(String, Vec<TextSplice>), String> {
    let literal: &str = analysis_bound(cursor_type);
    let (replaced, spans) =
        replace_intrinsics_with_spans(sql, MODEL_SQL_CONTEXT, |_| literal.to_owned())?;
    let splices: Vec<TextSplice> = spans
        .iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    Ok((replaced, splices))
}
