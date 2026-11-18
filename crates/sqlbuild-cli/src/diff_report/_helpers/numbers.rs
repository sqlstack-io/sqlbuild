//! Python's `{:,}` and `{:.2%}` number formats.

use sqlbuild_core::number_text::main::format_thousands::format_thousands;

/// An integer with comma thousands separators.
pub(crate) fn grouped(value: i64) -> String {
    format_thousands(i128::from(value))
}

/// A ratio as a percentage with two decimals.
pub(crate) fn percent(ratio: f64) -> String {
    format!("{:.2}%", ratio * 100.0)
}

/// `numerator / denominator` as a percentage, or `0.00%` without a denominator.
pub(crate) fn percentage(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return "0.00%".to_owned();
    }
    percent(numerator as f64 / denominator as f64)
}
