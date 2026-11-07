use std::collections::HashMap;

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::models::{LocatedGap, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::semantic_validation::main::normalize::normalize_analysis_sql;
use crate::semantic_validation::models::NormalizationInput;

/// The first gap of the native parser in `check`'s SQL, normalised as analysis reads it, with the
/// model's placeholder values filled; none when the parser reads it whole.
///
/// # Errors
///
/// The normalisation's message when the SQL cannot be normalised.
pub fn native_gap(
    dialect: &dyn NativeDialect,
    check: &SyntaxCheck,
) -> Result<Option<LocatedGap>, String> {
    let normalised: String = normalize_analysis_sql(NormalizationInput {
        sql: check.sql.clone(),
        dialect: dialect.name().to_owned(),
        stubs: HashMap::new(),
        placeholders: HashMap::new(),
    })?;
    let analysed: String = placeholder_defaults(&normalised, &check.placeholders);
    Ok(dialect
        .read_whole(&analysed)
        .err()
        .map(|gap| located(&analysed, gap)))
}

/// The gap with the line and column of its offset; an offset past the end is the end.
fn located(sql: &str, gap: ParserGap) -> LocatedGap {
    let before: &str = sql.get(..gap.offset).unwrap_or(sql);
    let line_start: usize = before.rfind('\n').map_or(0, |newline| newline + 1);
    LocatedGap {
        construct: gap.construct,
        line: before.matches('\n').count() + 1,
        column: before[line_start..].chars().count() + 1,
    }
}
