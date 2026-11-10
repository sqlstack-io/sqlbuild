use crate::analysis_route::constants::{GENERIC_DIALECT, SKIPPED_ANALYSIS_CODE};

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
