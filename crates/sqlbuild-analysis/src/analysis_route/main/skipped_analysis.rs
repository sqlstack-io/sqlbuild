use crate::analysis_route::constants::{GENERIC_DIALECT, SKIPPED_ANALYSIS_CODE};
use crate::analysis_route::models::LocatedGap;

/// P026's code and message for a model whose SQL the dialect's native parser cannot read, with
/// the parser's own message when it says more than the construct's name.
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

/// P026's code and message for a model whose dialect no native parser reads.
#[must_use]
pub fn unavailable_analysis(model: &str, dialect: Option<&str>) -> (&'static str, String) {
    (
        SKIPPED_ANALYSIS_CODE,
        format!(
            "SQL analysis skipped for model '{model}': no native parser reads the {} dialect",
            dialect.unwrap_or(GENERIC_DIALECT)
        ),
    )
}
