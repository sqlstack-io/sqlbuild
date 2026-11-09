use crate::analysis_route::_helpers::analysis::located_gap::sited_reason;
use crate::analysis_route::constants::{SKIPPED_ANALYSIS_CODE, SYNTAX_ERROR_CODE};
use crate::analysis_route::models::{GapSite, LocatedGap};

/// P026 for SQL the dialect's native parser cannot read; P001 for SQL it reports as invalid syntax.
#[must_use]
pub fn skipped_analysis(
    model: &str,
    dialect: &str,
    gap: &LocatedGap,
    site: &GapSite<'_>,
) -> (&'static str, String) {
    let reason: String = sited_reason(dialect, gap, site);
    if gap.syntax {
        return (
            SYNTAX_ERROR_CODE,
            format!("SQL syntax error in model '{model}'{reason}"),
        );
    }
    (
        SKIPPED_ANALYSIS_CODE,
        format!("SQL analysis skipped for model '{model}'{reason}"),
    )
}
