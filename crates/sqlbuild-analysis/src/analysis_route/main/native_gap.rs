use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::models::{LocatedGap, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::compiled_project::main::text_position::text_position;
use crate::compiled_project::models::TextPosition;
use crate::semantic_validation::main::normalize_native::normalize_native_sql;

/// The native parser's first gap in `check`'s analysis SQL, with the dialect's stub spelling.
pub fn native_gap(
    dialect: &dyn NativeDialect,
    check: &SyntaxCheck,
) -> Result<Option<LocatedGap>, String> {
    let normalised: String =
        normalize_native_sql(&check.sql, dialect.name(), dialect.stub_prefix())?;
    let analysed: String = placeholder_defaults(&normalised, &check.placeholders);
    Ok(dialect
        .read_whole(&analysed)
        .err()
        .map(|gap| located(&analysed, gap)))
}

/// The gap with the line and column of its offset; an offset past the end is the end.
fn located(sql: &str, gap: ParserGap) -> LocatedGap {
    let before: &str = sql.get(..gap.offset).unwrap_or(sql);
    let position: TextPosition = text_position(sql, before.chars().count());
    LocatedGap {
        construct: gap.construct,
        line: position.line,
        column: position.column,
    }
}
