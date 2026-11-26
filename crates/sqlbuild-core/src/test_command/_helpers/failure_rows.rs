//! A failed expectation as failure fields: what differs, then the differing rows.

use crate::progress::models::ExpectationStep;
use crate::progress::types::DifferenceSample;
use crate::run_output::models::{FailureField, FailureRow, RowTone};
use crate::test_command::_helpers::summary::expectation_summary;
use crate::test_command::constants::{ASSERTION_FAILED_REASON, ASSERTION_PREFIX, FAIL_OUTCOME};

/// The fields of one failed step: what differs and the rows; nothing when it passed or errored, whose error says why.
pub(crate) fn step_fields(step: &ExpectationStep) -> Vec<FailureField> {
    if step.outcome != FAIL_OUTCOME {
        return Vec::new();
    }
    if let Some(name) = step.model_name.strip_prefix(ASSERTION_PREFIX) {
        let mut fields: Vec<FailureField> = vec![FailureField::Error {
            code: None,
            message: format!("assertion {name} returned {}", expectation_summary(step)),
            help: None,
        }];
        let rows: Vec<FailureRow> = step
            .unexpected_samples
            .iter()
            .map(|sample| FailureRow {
                tone: RowTone::Error,
                text: sample_text(sample),
                note: String::new(),
            })
            .collect();
        if !rows.is_empty() {
            fields.push(FailureField::Rows(rows));
        }
        return fields;
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(missing) = step.missing_row_count.filter(|count| *count > 0) {
        parts.push(format!("{missing} missing"));
    }
    if let Some(extra) = step.unexpected_row_count.filter(|count| *count > 0) {
        parts.push(format!("{extra} extra"));
    }
    if parts.is_empty() {
        parts.push(format!("{} mismatched", step.mismatched_row_count));
    }
    let mut fields: Vec<FailureField> = vec![FailureField::Error {
        code: None,
        message: format!(
            "expected rows of {} differ: {} ({} expected, {} actual)",
            step.model_name,
            parts.join(", "),
            step.expected_row_count,
            step.actual_row_count
        ),
        help: None,
    }];
    let rows: Vec<FailureRow> = diff_rows(step);
    if !rows.is_empty() {
        fields.push(FailureField::Rows(rows));
    }
    fields
}

/// The short reason of a failed step: `3 rows differ`, or `assertion failed`.
pub(crate) fn step_reason(step: &ExpectationStep) -> String {
    if step.model_name.starts_with(ASSERTION_PREFIX) {
        return ASSERTION_FAILED_REASON.to_owned();
    }
    expectation_summary(step)
}

/// Column names, then `-` for each expected row missing and `+` for each unexpected row, aligned.
fn diff_rows(step: &ExpectationStep) -> Vec<FailureRow> {
    let samples = step
        .missing_samples
        .iter()
        .map(|sample| (RowTone::Expected, "-", "missing", sample))
        .chain(
            step.unexpected_samples
                .iter()
                .map(|sample| (RowTone::Actual, "+", "extra", sample)),
        );
    let samples: Vec<_> = samples.collect();
    let mut columns: Vec<String> = Vec::new();
    for (_, _, _, sample) in &samples {
        for (column, _) in sample.iter() {
            if !columns.contains(column) {
                columns.push(column.clone());
            }
        }
    }
    let sample_rows: Vec<&DifferenceSample> =
        samples.iter().map(|(_, _, _, sample)| *sample).collect();
    let widths: Vec<usize> = columns
        .iter()
        .map(|column| column_width(column, &sample_rows))
        .collect();
    let mut rows: Vec<FailureRow> = Vec::new();
    if !samples.is_empty() {
        rows.push(FailureRow {
            tone: RowTone::Header,
            text: diff_line(" ", &columns, &widths),
            note: String::new(),
        });
    }
    for (tone, sign, note, sample) in samples {
        let cells: Vec<String> = columns.iter().map(|column| cell(sample, column)).collect();
        rows.push(FailureRow {
            tone,
            text: diff_line(sign, &cells, &widths),
            note: note.to_owned(),
        });
    }
    for (column, actual, expected) in &step.column_differences {
        rows.push(FailureRow {
            tone: RowTone::Plain,
            text: format!("{column}: expected {expected}, actual {actual}"),
            note: "changed".to_owned(),
        });
    }
    rows
}

/// `column=value` for each cell of an assertion's sample row.
fn sample_text(sample: &DifferenceSample) -> String {
    sample
        .iter()
        .map(|(column, value)| format!("{column}={value}"))
        .collect::<Vec<String>>()
        .join("  ")
}

/// The value of `column` in a sample row, or nothing when the row lacks it.
fn cell(sample: &DifferenceSample, column: &str) -> String {
    sample
        .iter()
        .find(|(name, _)| name == column)
        .map_or_else(String::new, |(_, value)| value.clone())
}

/// The widest of `column`'s name and its values across the samples.
fn column_width(column: &str, samples: &[&DifferenceSample]) -> usize {
    samples
        .iter()
        .map(|sample| cell(sample, column).chars().count())
        .chain([column.chars().count()])
        .max()
        .unwrap_or(0)
}

/// `sign`, then each cell padded to its column's width.
fn diff_line(sign: &str, cells: &[String], widths: &[usize]) -> String {
    let cells: Vec<String> = cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| format!("{cell:<width$}"))
        .collect();
    format!("{sign}  {}", cells.join("  "))
        .trim_end()
        .to_owned()
}
