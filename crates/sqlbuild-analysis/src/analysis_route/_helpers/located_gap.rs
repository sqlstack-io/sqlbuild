//! Where a native parser's gap is, and why the parser cannot read the SQL there.

use crate::analysis_route::models::{LocatedGap, ParserGap};
use crate::compiled_project::main::text_position::text_position;
use crate::compiled_project::models::TextPosition;

/// The gap with the line and column of its offset; an offset past the end is the end.
pub(crate) fn located(sql: &str, gap: ParserGap) -> LocatedGap {
    let before: &str = sql.get(..gap.offset).unwrap_or(sql);
    let position: TextPosition = text_position(sql, before.chars().count());
    LocatedGap {
        construct: gap.construct,
        message: gap.message,
        line: position.line,
        column: position.column,
        syntax: gap.syntax,
    }
}

/// Why the dialect's native parser cannot read SQL, as P026 words it.
pub(crate) fn unreadable_reason(dialect: &str, gap: &LocatedGap) -> String {
    let detail: String = if gap.message.is_empty() || gap.message == gap.construct {
        String::new()
    } else {
        format!(": {}", gap.message)
    };
    format!(
        "the {dialect} parser cannot read {} at line {}, column {}{detail}",
        gap.construct, gap.line, gap.column
    )
}
