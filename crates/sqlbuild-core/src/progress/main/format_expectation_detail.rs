//! The failing row detail of one SQL expectation row.

use crate::progress::constants::{
    ASSERTION_PREFIX, ERROR_OUTCOME, PASS_OUTCOME, REFUSED_OUTCOME, TIMEOUT_OUTCOME,
};
use crate::progress::models::ExpectationStep;
use crate::progress::types::DifferenceSample;

/// Python's rendering of an optional count: the number, or `None`.
fn optional_count(count: Option<i64>) -> String {
    count.map_or_else(|| "None".to_owned(), |count| count.to_string())
}

fn sample_lines(label: &str, samples: &[DifferenceSample]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        let values: Vec<String> = sample
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        lines.push(format!("{label} {}: {}", index + 1, values.join(", ")));
    }
    lines
}

/// Row counts, samples and differing columns of a failing step; empty when it did not run to a comparison.
#[must_use]
pub fn format_expectation_detail(step: &ExpectationStep) -> String {
    if [
        PASS_OUTCOME,
        ERROR_OUTCOME,
        TIMEOUT_OUTCOME,
        REFUSED_OUTCOME,
    ]
    .contains(&step.outcome.as_str())
    {
        return String::new();
    }
    if step.model_name.starts_with(ASSERTION_PREFIX) {
        let row_label: &str = if step.actual_row_count == 1 {
            "row"
        } else {
            "rows"
        };
        return format!("  {} {row_label}", step.actual_row_count);
    }
    if step.unexpected_row_count.is_none()
        && step.missing_row_count.is_none()
        && step.unexpected_samples.is_empty()
        && step.missing_samples.is_empty()
    {
        return format!("  {} mismatched", step.mismatched_row_count);
    }
    let mut parts: Vec<String> = vec![
        format!("unexpected={}", optional_count(step.unexpected_row_count)),
        format!("missing={}", optional_count(step.missing_row_count)),
    ];
    if step.actual_row_count != step.expected_row_count {
        parts.push(format!(
            "row counts actual={} expected={}",
            step.actual_row_count, step.expected_row_count
        ));
    }
    parts.extend(sample_lines("unexpected sample", &step.unexpected_samples));
    parts.extend(sample_lines("missing sample", &step.missing_samples));
    for (name, actual, expected) in &step.column_differences {
        parts.push(format!(
            "differing column {name}: actual={actual}, expected={expected}"
        ));
    }
    format!("  {}", parts.join("; "))
}
