use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::_helpers::analysis::located_gap::located;
use crate::analysis_route::models::LocatedGap;
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
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
        .map(|gap| located(&analysed, gap, Some(&check.sql))))
}
