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
