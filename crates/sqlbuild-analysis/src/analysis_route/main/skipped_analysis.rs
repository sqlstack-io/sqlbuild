use crate::analysis_route::_helpers::located_gap::unreadable_reason;
use crate::analysis_route::constants::SKIPPED_ANALYSIS_CODE;
use crate::analysis_route::models::LocatedGap;

/// P026's code and message for a model whose SQL the dialect's native parser cannot read.
#[must_use]
pub fn skipped_analysis(model: &str, dialect: &str, gap: &LocatedGap) -> (&'static str, String) {
    (
        SKIPPED_ANALYSIS_CODE,
        format!(
            "SQL analysis skipped for model '{model}': {}",
            unreadable_reason(dialect, gap)
        ),
    )
}
