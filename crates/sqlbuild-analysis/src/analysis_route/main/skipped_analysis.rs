use crate::analysis_route::constants::SKIPPED_ANALYSIS_CODE;
use crate::analysis_route::models::LocatedGap;

/// P026's code and message for a model whose SQL the dialect's native parser cannot read.
#[must_use]
pub fn skipped_analysis(model: &str, dialect: &str, gap: &LocatedGap) -> (&'static str, String) {
    let detail: String = if gap.message.is_empty() || gap.message == gap.construct {
        String::new()
    } else {
        format!(": {}", gap.message)
    };
    (
        SKIPPED_ANALYSIS_CODE,
        format!(
            "SQL analysis skipped for model '{model}': the {dialect} parser cannot read {} at \
             line {}, column {}{detail}",
            gap.construct, gap.line, gap.column
        ),
    )
}
